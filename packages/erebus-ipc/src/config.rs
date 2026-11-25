//! The frontend writes the config and theme binaries here, then sends
//! [`FrontendMessage::Sync`](crate::FrontendMessage::Sync) so backends reload them.
//!
//! ```text
//! <config_dir>/erebus/
//! - config.toml
//! - themes/
//!     - <id>.wasm
//! ```

use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Path, PathBuf},
};

use erebus_theme::settings::SettingValue;
use serde::{Deserialize, Serialize};

/// The launcher sets this to the app's config ID. Without it, the backend checks
/// [`APP_ID_MARKER`] before falling back to the executable name.
pub const APP_ID_ENV: &str = "EREBUS_APP";

/// Use a marker when the frontend can't set the app's launch environment. The first line
/// holds the app ID; the frontend can use the remaining lines for its own bookkeeping.
///
/// For versioned installs, keep the marker above the `app-<version>` folders so it survives
/// app updates.
pub const APP_ID_MARKER: &str = "d3d11.erebus";

/// Look beside the executable first. Apps in an `app-<version>` folder can also have
/// their marker in the parent directory.
pub fn marked_app_id(executable: &Path) -> Option<String> {
    let dir = executable.parent()?;
    let versions = dir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.starts_with("app-"))
        .and_then(|_| dir.parent());

    let text = std::iter::once(dir)
        .chain(versions)
        .find_map(|dir| fs::read_to_string(dir.join(APP_ID_MARKER)).ok())?;
    let id = text.lines().next()?.trim();
    (!id.is_empty()).then(|| id.to_owned())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigStore {
    /// This order determines how enabled themes are applied to each app.
    #[serde(default)]
    pub themes: Vec<ThemeEntry>,
    /// Keys match the ID reported by the backend, resolved from [`APP_ID_ENV`],
    /// [`APP_ID_MARKER`], or the executable name, in that order.
    #[serde(default)]
    pub apps: BTreeMap<String, AppConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeEntry {
    /// file stem of the theme's binary in [`themes_dir`]
    pub id: String,
    /// Keep the original path so the frontend can watch for theme updates.
    pub source: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    /// shown in frontends in place of the ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// an icon name or path for frontends to show
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Remember which installed app was injected, e.g. by its desktop file ID.
    /// Apps discovered through a backend connection may not have this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_app: Option<String>,
    /// keyed by theme id. Themes missing here are disabled
    #[serde(default)]
    pub themes: BTreeMap<String, AppTheme>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppTheme {
    pub enabled: bool,
    #[serde(default)]
    pub settings: HashMap<String, SettingValue>,
}

impl ThemeEntry {
    pub fn binary_path(&self) -> io::Result<PathBuf> {
        Ok(themes_dir()?.join(&self.id).with_extension("wasm"))
    }
}

pub fn config_dir() -> io::Result<PathBuf> {
    flatpak_host_config_dir()
        .or_else(dirs::config_dir)
        .map(|dir| dir.join("erebus"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config directory"))
}

/// Flatpak redirects the app's config directory into `~/.var/app/<id>`. Erebus still
/// needs the host's config, which injection makes readable inside the sandbox.
/// Use `HOST_XDG_CONFIG_HOME` to find it, falling back to `~/.config`.
fn flatpak_host_config_dir() -> Option<PathBuf> {
    let sandboxed =
        std::env::var_os("FLATPAK_ID").is_some() || Path::new("/.flatpak-info").exists();
    if !sandboxed {
        return None;
    }

    std::env::var_os("HOST_XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| Some(dirs::home_dir()?.join(".config")))
}

pub fn themes_dir() -> io::Result<PathBuf> {
    Ok(config_dir()?.join("themes"))
}

fn state_path() -> io::Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

impl ConfigStore {
    /// A missing config is normal on the first run; start with an empty store.
    pub fn load() -> io::Result<Self> {
        match fs::read(state_path()?) {
            Ok(bytes) => toml::from_slice(&bytes)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err),
        }
    }

    pub fn save(&self) -> io::Result<()> {
        write_atomic(
            &state_path()?,
            toml::to_string_pretty(self)
                .map_err(io::Error::other)?
                .as_bytes(),
        )
    }
}

/// Write to a temporary file before renaming it into place so readers never see a
/// partially written config or theme binary.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp = path.with_extension("tmp");
    let written = fs::write(&tmp, bytes).and_then(|()| fs::rename(&tmp, path));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}
