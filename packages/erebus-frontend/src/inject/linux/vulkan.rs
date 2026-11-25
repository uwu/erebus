//! Register the backend as an implicit Vulkan layer, enabled by [`ENABLE_ENV`].
//! Launching with `env EREBUS_ENABLE=1 app` then loads erebus without having to merge
//! an explicit layer list with whatever the launcher already requests.
//!
//! ```text
//! <data_dir>/
//! - erebus/vulkan/liberebus_vulkan.so (installed backend)
//! - erebus/vulkan/bundled.stamp (identifies the bundled copy it came from)
//! - vulkan/implicit_layer.d/erebus.json (manifest pointing to the installed backend)
//! ```

use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
};

use erebus_ipc::config::write_atomic;

use crate::inject::stamp;

pub const NAME: &str = "Vulkan layer";
/// the name of the layer, which has to match across every manifest for it
pub const LAYER_NAME: &str = "VK_LAYER_erebus";
/// set to `1` to load the layer into an app
pub const ENABLE_ENV: &str = "EREBUS_ENABLE";
/// set to `1` to keep the layer out of an app even when [`ENABLE_ENV`] is set
pub const DISABLE_ENV: &str = "EREBUS_DISABLE";
/// points at a layer to use instead of the bundled one, for development
pub const OVERRIDE_ENV: &str = "EREBUS_VULKAN_LAYER";

const LIBRARY: &str = "liberebus_vulkan.so";
/// where the layer is installed, relative to `$XDG_DATA_HOME`
pub const INSTALL_DIR: &str = "erebus/vulkan";

/// Where a frontend looks for the layer it was shipped with, in order.
fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(path) = env::var_os(OVERRIDE_ENV) {
        paths.push(PathBuf::from(path));
    }

    if let Some(dir) = env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.to_owned()))
    {
        // next to the binary: cargo's target directory, or an unpacked release archive
        paths.push(dir.join(LIBRARY));
        // a distro package with the binary in /usr/bin
        paths.push(dir.join("../lib/erebus").join(LIBRARY));
    }

    paths
}

/// The layer couldn't be found anywhere the frontend looks for it.
#[derive(Debug, Clone)]
pub struct NotBundled {
    pub searched: Vec<PathBuf>,
}

impl fmt::Display for NotBundled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "couldn't find {LIBRARY} (looked for ")?;
        for (index, path) in self.searched.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", path.display())?;
        }
        f.write_str(")")
    }
}

impl std::error::Error for NotBundled {}

pub fn bundled() -> Result<PathBuf, NotBundled> {
    let searched = candidates();
    searched
        .iter()
        .find(|path| path.is_file())
        .map(|path| path.canonicalize().unwrap_or_else(|_| path.clone()))
        .ok_or(NotBundled { searched })
}

fn data_dir() -> io::Result<PathBuf> {
    dirs::data_dir().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no data directory"))
}

pub fn installed_library() -> io::Result<PathBuf> {
    Ok(library_in(&data_dir()?))
}

/// Where the installed copy of the layer lives, for a given `$XDG_DATA_HOME`.
pub fn library_in(data_home: &Path) -> PathBuf {
    data_home.join(INSTALL_DIR).join(LIBRARY)
}

/// Where the loader finds the layer's manifest.
pub fn manifest_path() -> io::Result<PathBuf> {
    Ok(data_dir()?.join("vulkan/implicit_layer.d/erebus.json"))
}

pub fn is_installed() -> bool {
    manifest_path().is_ok_and(|path| path.is_file())
}

/// Install and register the bundled layer unless this copy is already installed. Return whether
/// it changed.
///
/// Replace the file instead of overwriting it: running apps still have the old copy mapped and
/// will pick up the new one on restart.
pub fn install(bundled: &Path) -> io::Result<bool> {
    let library = installed_library()?;
    let manifest_path = manifest_path()?;
    let stamp_path = library.with_file_name("bundled.stamp");

    let stamp = stamp(bundled)?;
    let library_changed = !library.is_file()
        || fs::read_to_string(&stamp_path).map_or(true, |installed| installed != stamp);
    if library_changed {
        write_atomic(&library, &fs::read(bundled)?)?;
        write_atomic(&stamp_path, stamp.as_bytes())?;
    }

    let manifest = manifest(&library)?;
    let manifest_changed =
        fs::read_to_string(&manifest_path).map_or(true, |installed| installed != manifest);
    if manifest_changed {
        write_atomic(&manifest_path, manifest.as_bytes())?;
    }

    Ok(library_changed || manifest_changed)
}

/// Update an existing installation to match this frontend. Don't create one until the user
/// injects an app.
pub fn update() -> io::Result<bool> {
    if !is_installed() {
        return Ok(false);
    }

    match bundled() {
        Ok(bundled) => install(&bundled),
        // Keep the installed layer if this frontend has no bundled replacement.
        Err(_) => Ok(false),
    }
}

/// The manifest that registers `library` as the layer.
pub fn manifest(library: &Path) -> io::Result<String> {
    let library = library.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} isn't valid UTF-8", library.display()),
        )
    })?;

    Ok(format!(
        r#"{{
  "file_format_version": "1.2.0",
  "layer": {{
    "name": "{LAYER_NAME}",
    "type": "GLOBAL",
    "library_path": {library},
    "api_version": "1.3.0",
    "implementation_version": "1",
    "description": "Theme engine for GPUI applications",
    "functions": {{
      "vkNegotiateLoaderLayerInterfaceVersion": "vkNegotiateLoaderLayerInterfaceVersion"
    }},
    "enable_environment": {{ "{ENABLE_ENV}": "1" }},
    "disable_environment": {{ "{DISABLE_ENV}": "1" }}
  }}
}}
"#,
        library = json_string(library),
    ))
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for char in value.chars() {
        match char {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            char if char.is_control() => out.push_str(&format!("\\u{:04x}", char as u32)),
            char => out.push(char),
        }
    }
    out.push('"');
    out
}
