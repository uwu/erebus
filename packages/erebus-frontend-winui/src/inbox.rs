//! Gets events from background threads (IPC and file watching) over to the UI.
//!
//! Reactor only lets other threads send messages as the result of background work, so the
//! UI keeps one background task parked here. When something arrives, it hands back
//! everything that's queued up and the UI starts another one.

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, PoisonError,
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    time::Duration,
};

use erebus_frontend::ipc::Event;
use windows_reactor::CancellationToken;

/// how often a parked task checks whether it's been cancelled
const CANCEL_POLL: Duration = Duration::from_millis(250);

pub enum ThreadEvent {
    Ipc(Event),
    SourcesChanged(Vec<PathBuf>),
}

#[derive(Clone)]
pub struct Inbox {
    sender: Sender<ThreadEvent>,
    receiver: Arc<Mutex<Receiver<ThreadEvent>>>,
}

impl Inbox {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }

    pub fn sender(&self) -> Sender<ThreadEvent> {
        self.sender.clone()
    }

    /// Blocks until something arrives then returns everything in the queue.
    /// 
    /// Returns None if `cancel` gets cancelled first.
    pub fn wait(&self, cancel: &CancellationToken) -> Vec<ThreadEvent> {
        let receiver = self.receiver.lock().unwrap_or_else(PoisonError::into_inner);

        loop {
            match receiver.recv_timeout(CANCEL_POLL) {
                Ok(first) => return std::iter::once(first).chain(receiver.try_iter()).collect(),
                Err(RecvTimeoutError::Timeout) if !cancel.is_cancelled() => {}
                Err(_) => return Vec::new(),
            }
        }
    }
}
