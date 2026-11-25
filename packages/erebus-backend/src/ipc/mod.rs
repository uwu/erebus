//! erebus backend IPC client implementation

mod logging;

pub use logging::init as init_logging;

use std::{
    io,
    sync::{LazyLock, atomic::AtomicBool},
    thread,
    time::Duration,
};

use interprocess::local_socket::{Stream, traits::Stream as _};

use erebus_ipc::{
    config::{APP_ID_ENV, marked_app_id},
    protocol::{FrontendMessage, Handshake, HandshakeReply, SUPPORTED_VERSIONS},
    transport,
};

/// how long to wait before trying to reach a frontend again
const RECONNECT_INTERVAL: Duration = Duration::from_secs(2);

/// The ID of the app we're running in, which is what apps are keyed by in the config
/// store: whatever launched the app set [`APP_ID_ENV`], what a frontend left in the app's
/// [`APP_ID_MARKER`](erebus_ipc::config::APP_ID_MARKER), or the name of the executable.
pub fn app_id() -> &'static str {
    static ID: LazyLock<String> = LazyLock::new(|| {
        std::env::var(APP_ID_ENV)
            .ok()
            .filter(|id| !id.is_empty())
            .or_else(|| {
                let exe = std::env::current_exe().ok()?;
                marked_app_id(&exe)
                    .or_else(|| Some(exe.file_name()?.to_string_lossy().into_owned()))
            })
            .unwrap_or_default()
    });

    &ID
}

pub struct IpcBackend {
    /// shown in frontends, e.g. `vulkan`
    backend: String,
}

impl IpcBackend {
    pub fn new(backend: impl Into<String>) -> Self {
        Self {
            backend: backend.into(),
        }
    }

    /// Spawns a background thread for talking to frontends
    pub fn spawn(self, mut on_sync: impl FnMut() + Send + 'static) -> io::Result<()> {
        thread::Builder::new()
            .name("erebus-ipc".into())
            .spawn(move || {
                // only report a failure to connect when it's different from the last one so a missing
                // frontend doesn't log every few seconds
                let mut last_error = None;

                loop {
                    match self.serve(&mut on_sync) {
                        Ok(()) => last_error = None,
                        Err(err) => {
                            let err = err.to_string();
                            if last_error.as_ref() != Some(&err) {
                                log::error!("couldn't connect to a frontend: {err}");
                                last_error = Some(err);
                            }
                        }
                    }

                    thread::sleep(RECONNECT_INTERVAL);
                }
            })?;

        Ok(())
    }

    /// talks to a frontend until it goes away. Only returns `Err` if it couldn't connect.
    fn serve(&self, on_sync: &mut impl FnMut()) -> io::Result<()> {
        let name = transport::socket_name()?;
        let stream = Stream::connect(name.borrow())
            .map_err(|err| io::Error::new(err.kind(), format!("{name:?}: {err}")))?;

        transport::send(
            &stream,
            &Handshake {
                versions: SUPPORTED_VERSIONS,
                process: app_id().to_owned(),
                pid: std::process::id(),
                backend: self.backend.clone(),
            },
        )?;

        match transport::recv(&stream)? {
            HandshakeReply::Accept { version } => {
                log::info!("connected to frontend (protocol v{version})")
            }
            HandshakeReply::Reject => {
                return Err(io::Error::other("frontend rejected our protocol versions"));
            }
        }

        // the frontend may have changed things since we last loaded them
        on_sync();

        let closed = AtomicBool::new(false);
        let ended = thread::scope(|scope| {
            scope.spawn(|| logging::forward(&stream, &closed));

            let ended = loop {
                match transport::recv(&stream) {
                    Ok(FrontendMessage::Sync) => on_sync(),
                    Err(err) => break err,
                }
            };

            logging::close(&closed);
            ended
        });

        match ended.kind() {
            io::ErrorKind::UnexpectedEof => log::info!("disconnected from frontend"),
            _ => log::warn!("lost connection to frontend: {ended}"),
        }

        Ok(())
    }
}
