use std::{cell::RefCell, collections::VecDeque, rc::Rc, time::SystemTime};

use chrono::{DateTime, Local};
use erebus_frontend::{
    apps::{BackendInstance, LogStorage},
    ipc::Event,
};
use erebus_ipc::protocol::LogRecord;
use log::Level;
use zgui::prelude::*;

use super::AppState;

/// Older records are dropped past this, so the window stays responsive.
const CAPACITY: usize = 5000;

pub struct Line {
    pub id: u64,
    pub level: Level,
    /// time and pid
    pub origin: String,
    pub target: String,
    pub message: String,
}

struct Buffer {
    lines: VecDeque<Rc<Line>>,
    next: u64,
    max_level: Level,
}

#[derive(Clone)]
pub struct LogBuffer(Rc<RefCell<Buffer>>);

impl Default for LogBuffer {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(Buffer {
            lines: VecDeque::new(),
            next: 0,
            max_level: Level::Trace,
        })))
    }
}

impl LogStorage for LogBuffer {
    fn push(&self, pid: u32, record: &LogRecord) {
        let time = DateTime::from_timestamp_millis(record.timestamp as i64)
            .map(|time| {
                time.with_timezone(&Local)
                    .format("%H:%M:%S%.3f")
                    .to_string()
            })
            .unwrap_or_default();

        let mut buffer = self.0.borrow_mut();
        let line = Line {
            id: buffer.next,
            level: record.level,
            origin: format!("{time} [{pid}]"),
            target: format!("{}:", record.target),
            message: record.message.clone(),
        };
        buffer.next += 1;
        buffer.lines.push_back(Rc::new(line));
        if buffer.lines.len() > CAPACITY {
            buffer.lines.pop_front();
        }
    }
}

impl LogBuffer {
    pub fn visible(&self, max: Level) -> Vec<Rc<Line>> {
        self.0
            .borrow()
            .lines
            .iter()
            .filter(|line| line.level <= max)
            .cloned()
            .collect()
    }

    pub fn max_level(&self) -> Level {
        self.0.borrow().max_level
    }

    pub fn set_max_level(&self, level: Level) {
        self.0.borrow_mut().max_level = level;
    }

    pub fn clear(&self) {
        self.0.borrow_mut().lines.clear();
    }
}

impl AppState {
    pub fn instances(self, app_id: &str) -> usize {
        self.runtime.with(|runtime| runtime.instances(app_id).len())
    }

    pub(super) fn handle_event(self, event: Event) {
        match event {
            Event::Connected {
                peer,
                process: app_id,
                backend,
                pid,
                version,
            } => {
                self.runtime.update(|runtime| {
                    runtime.connect(
                        app_id.clone(),
                        BackendInstance {
                            peer,
                            pid,
                            backend,
                            version,
                            connected_at: SystemTime::now(),
                        },
                    )
                });

                if self.edit(|library| library.register_app(&app_id)) {
                    self.save();
                }
            }

            Event::Disconnected { peer } => {
                self.runtime.update(|runtime| {
                    runtime.disconnect(peer);
                });
                self.fix_selection();
            }

            Event::Log { peer, record } => {
                self.runtime
                    .with_untracked(|runtime| runtime.log(peer, &record));
                self.logs_changed.notify();
            }
        }
    }
}
