//! The frontend ships with a backend built from the same commit so their protocols match.
//! We install that backend in the user's data directory before injecting any apps. This
//! gives it a stable home even if the frontend moves. Later frontend updates replace it.
//!
//! The backend also needs to know which app config to use. We pass the app ID through
//! [`APP_ID_ENV`](erebus_ipc::config::APP_ID_ENV) when we control the launch environment.
//! Otherwise we leave an [`APP_ID_MARKER`](erebus_ipc::config::APP_ID_MARKER) beside it.
//! The matching [`AppConfig`](erebus_ipc::config::AppConfig) records which installed app
//! was injected.

use std::{fs, io, path::Path};

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "linux")]
pub use linux::{installed, vulkan as backend};
#[cfg(windows)]
pub use windows::{d3d11 as backend, installed};

/// Use the path, size and modification time to recognize a bundled backend without reading the
/// whole library on every check.
pub(crate) fn stamp(bundled: &Path) -> io::Result<String> {
    let meta = fs::metadata(bundled)?;
    let modified = meta
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(format!(
        "{}\n{}\n{}\n",
        bundled.display(),
        meta.len(),
        modified.as_nanos()
    ))
}
