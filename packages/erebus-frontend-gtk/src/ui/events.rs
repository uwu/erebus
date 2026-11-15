//! event handling from IPC

use std::{rc::Rc, time::SystemTime};

use erebus_frontend::{apps::BackendInstance, ipc::Event};

use super::app::{App, Selection};

impl App {
    pub fn handle_event(self: &Rc<Self>, event: Event) {
        match event {
            Event::Connected {
                peer,
                process,
                backend,
                pid,
                version,
            } => {
                self.runtime.borrow_mut().connect(
                    process.clone(),
                    BackendInstance {
                        peer,
                        pid,
                        backend,
                        version,
                        connected_at: SystemTime::now(),
                    },
                );

                let registered = self.library.borrow_mut().register_app(&process);
                if registered {
                    self.save();
                }

                self.rebuild_sidebar();
                self.refresh(&Selection::App(process));
            }

            Event::Disconnected { peer } => {
                let process = self.runtime.borrow_mut().disconnect(peer);
                let Some(process) = process else {
                    return;
                };

                self.rebuild_sidebar();
                self.refresh(&Selection::App(process));
            }

            Event::Log { peer, record } => self.runtime.borrow().log(peer, &record),
        }
    }
}
