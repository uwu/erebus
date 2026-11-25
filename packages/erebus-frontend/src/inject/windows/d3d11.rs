//! Keep an installed copy of the bundled `d3d11.dll` as the source for app injections
//! (see [`installed`](super::installed)). Each app needs a copy beside its executable.
//! Updating the installed copy changes its stamp so we can find apps with older copies.
//!
//! Apps also get a copy of the system DLL named [`ORIGINAL`] for forwarded functions.
//! The backend itself loads the real DLL from System32.
//!
//! ```text
//! <local_data_dir>/
//! - erebus/d3d11/d3d11.dll (installed backend)
//! - erebus/d3d11/bundled.stamp (identifies the bundled copy it came from)
//! ```

use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
};

use erebus_ipc::config::write_atomic;

use crate::inject::stamp;

pub const NAME: &str = "Direct3D 11 library";
/// points at a library to use instead of the bundled one, for development
pub const OVERRIDE_ENV: &str = "EREBUS_D3D11";

pub const LIBRARY: &str = "d3d11.dll";
/// what a copy of the system's library is called next to an app
pub const ORIGINAL: &str = "d3d11_orig.dll";
/// where the library is installed, relative to the local data directory
pub const INSTALL_DIR: &str = r"erebus\d3d11";

/// Where a frontend looks for the library it was shipped with, in order.
fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(path) = env::var_os(OVERRIDE_ENV) {
        paths.push(PathBuf::from(path));
    }

    // next to the binary: cargo's target directory, or an unpacked release archive
    if let Some(dir) = env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.to_owned()))
    {
        paths.push(dir.join(LIBRARY));
    }

    paths
}

/// The library couldn't be found anywhere the frontend looks for it.
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

fn install_dir() -> io::Result<PathBuf> {
    dirs::data_local_dir()
        .map(|dir| dir.join(INSTALL_DIR))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no local data directory"))
}

pub fn installed_library() -> io::Result<PathBuf> {
    Ok(install_dir()?.join(LIBRARY))
}

fn stamp_path() -> io::Result<PathBuf> {
    Ok(install_dir()?.join("bundled.stamp"))
}

/// Which bundled library the installed one was made from, if one was installed.
pub fn installed_stamp() -> Option<String> {
    fs::read_to_string(stamp_path().ok()?).ok()
}

pub fn is_installed() -> bool {
    installed_library().is_ok_and(|path| path.is_file())
}

/// The library this replaces, which [`ORIGINAL`] is a copy of.
pub fn system_library() -> PathBuf {
    let root = env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    Path::new(&root).join("System32").join(LIBRARY)
}

/// Install the bundled library unless this copy is already installed. Return whether it
/// changed.
pub fn install(bundled: &Path) -> io::Result<bool> {
    let library = installed_library()?;
    let stamp_path = stamp_path()?;

    let stamp = stamp(bundled)?;
    let changed = !library.is_file()
        || fs::read_to_string(&stamp_path).map_or(true, |installed| installed != stamp);
    if changed {
        write_atomic(&library, &fs::read(bundled)?)?;
        write_atomic(&stamp_path, stamp.as_bytes())?;
    }

    Ok(changed)
}

/// Update an existing installation to match this frontend. Don't create one until the user
/// injects an app.
pub fn update() -> io::Result<bool> {
    if !is_installed() {
        return Ok(false);
    }

    match bundled() {
        Ok(bundled) => install(&bundled),
        // Keep the installed library if this frontend has no bundled replacement.
        Err(_) => Ok(false),
    }
}

/// Load the system DLL before initializing graphics. Otherwise a renderer loading d3d11 by name
/// could find the bundled backend beside the frontend.
pub fn load_system_library() {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::LibraryLoader::LoadLibraryW;

    let path: Vec<u16> = system_library()
        .as_os_str()
        .encode_wide()
        .chain([0])
        .collect();
    // SAFETY: the path ends in a NUL, and the library is never freed
    unsafe { LoadLibraryW(path.as_ptr()) };
}
