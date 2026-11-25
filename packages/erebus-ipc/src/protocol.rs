use std::ops::RangeInclusive;

/// Also versions the on-disk [`ConfigStore`](crate::config::ConfigStore) format.
pub const SUPPORTED_VERSIONS: RangeInclusive<u32> = 1..=1;

//
// MARK: handshake
//

/// The first message a backend sends after connecting.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Handshake {
    pub versions: RangeInclusive<u32>,
    /// Despite the field name, this is the app's config ID. The backend resolves it
    /// from [`APP_ID_ENV`](crate::config::APP_ID_ENV), a marker, or the executable name.
    pub process: String,
    /// Identifies the backend, such as Vulkan or Direct3D 11.
    pub backend: String,
    pub pid: u32,
}

/// The frontend's reply to a [`Handshake`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HandshakeReply {
    Accept { version: u32 },
    Reject,
}

impl HandshakeReply {
    /// Use the newest shared version, or reject the connection if the ranges don't overlap.
    pub fn negotiate_version(handshake: &Handshake) -> Self {
        let lo = *SUPPORTED_VERSIONS.start().max(handshake.versions.start());
        let hi = *SUPPORTED_VERSIONS.end().min(handshake.versions.end());

        if lo <= hi {
            Self::Accept { version: hi }
        } else {
            Self::Reject
        }
    }
}

//
// MARK: messages
//

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FrontendMessage {
    /// Reload the config and themes from disk; either may have changed.
    Sync,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum BackendMessage {
    Log(LogRecord),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogRecord {
    pub level: log::Level,
    pub target: String,
    pub message: String,
    /// milliseconds since the unix epoch
    pub timestamp: u64,
}
