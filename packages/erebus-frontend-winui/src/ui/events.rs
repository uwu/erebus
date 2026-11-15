//! Handles whatever arrives from background threads: IPC events and theme file changes.

use std::time::SystemTime;

use erebus_frontend::{apps::BackendInstance, ipc::Event};
use windows_reactor::*;

use super::Erebus;
use crate::inbox::ThreadEvent;

impl Erebus {
    pub(super) fn handle_thread_event(
        &mut self,
        event: &ThreadEvent,
        context: &ComponentContext<Self>,
    ) {
        match event {
            ThreadEvent::Ipc(Event::Connected {
                peer,
                process,
                backend,
                pid,
                version,
            }) => {
                self.runtime.connect(
                    process.clone(),
                    BackendInstance {
                        peer: *peer,
                        pid: *pid,
                        backend: backend.clone(),
                        version: *version,
                        connected_at: SystemTime::now(),
                    },
                );

                if self.library.register_app(process) {
                    self.save(context);
                }
            }
            ThreadEvent::Ipc(Event::Disconnected { peer }) => {
                self.runtime.disconnect(*peer);
            }
            ThreadEvent::Ipc(Event::Log { peer, record }) => self.runtime.log(*peer, record),
            ThreadEvent::SourcesChanged(paths) => self.reload_sources(paths, context),
        }
    }
}
