//! What we know about running apps since the frontend started.

use std::{collections::HashMap, time::SystemTime};

use erebus_ipc::protocol::LogRecord;

use crate::ipc::PeerId;

pub trait LogStorage: Clone + Default {
    fn push(&self, pid: u32, record: &LogRecord);
}

/// A running process of a backend.
pub struct BackendInstance {
    pub peer: PeerId,
    pub pid: u32,
    pub backend: String,
    /// protocol version
    pub version: u32,
    pub connected_at: SystemTime,
}

struct AppRuntime<L> {
    instances: Vec<BackendInstance>,
    logs: L,
}

impl<L: LogStorage> AppRuntime<L> {
    fn new() -> Self {
        Self {
            instances: Vec::new(),
            logs: L::default(),
        }
    }
}

pub struct Runtime<L> {
    apps: HashMap<String, AppRuntime<L>>,
    /// which app each connected peer belongs to
    peers: HashMap<PeerId, String>,
}

impl<L> Default for Runtime<L> {
    fn default() -> Self {
        Self {
            apps: HashMap::new(),
            peers: HashMap::new(),
        }
    }
}

impl<L: LogStorage> Runtime<L> {
    pub fn connect(&mut self, process: String, instance: BackendInstance) {
        self.peers.insert(instance.peer, process.clone());
        self.apps
            .entry(process)
            .or_insert_with(AppRuntime::new)
            .instances
            .push(instance);
    }

    /// Returns the app the peer belonged to, if it was connected.
    pub fn disconnect(&mut self, peer: PeerId) -> Option<String> {
        let process = self.peers.remove(&peer)?;
        if let Some(app) = self.apps.get_mut(&process) {
            app.instances.retain(|instance| instance.peer != peer);
        }
        Some(process)
    }

    pub fn log(&self, peer: PeerId, record: &LogRecord) {
        let Some(app) = self
            .peers
            .get(&peer)
            .and_then(|process| self.apps.get(process))
        else {
            return;
        };

        if let Some(instance) = app.instances.iter().find(|instance| instance.peer == peer) {
            app.logs.push(instance.pid, record);
        }
    }

    /// Drops everything known about an app.
    pub fn forget(&mut self, process: &str) {
        self.apps.remove(process);
    }

    pub fn instances(&self, process: &str) -> &[BackendInstance] {
        self.apps.get(process).map_or(&[], |app| &app.instances)
    }

    /// Apps with at least one instance.
    pub fn running(&self) -> impl Iterator<Item = &String> {
        self.apps
            .iter()
            .filter(|(_, app)| !app.instances.is_empty())
            .map(|(process, _)| process)
    }

    pub fn logs(&mut self, process: &str) -> L {
        self.apps
            .entry(process.to_owned())
            .or_insert_with(AppRuntime::new)
            .logs
            .clone()
    }
}
