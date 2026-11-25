//! Flatpak controls the app's environment and filesystem access, so editing its desktop
//! entry isn't enough. We write a per-user [override] in
//! `$XDG_DATA_HOME/flatpak/overrides/<app>` to set `EREBUS_ENABLE` and `EREBUS_APP`
//! and grant access to the paths from [`filesystems`]. This also covers terminal and D-Bus launches.
//!
//! The Vulkan loader inside the sandbox needs a manifest in its own `$XDG_DATA_HOME`.
//! We put that in `~/.var/app/<app>/data/vulkan/implicit_layer.d`, pointing to the
//! installed layer at the path made visible by the override.
//!
//! The user may already have overrides. Leave those alone and only change the keys
//! and filesystem permissions erebus adds.
//!
//! [override]: https://docs.flatpak.org/en/latest/flatpak-command-reference.html#flatpak-override

use std::{fs, io, path::PathBuf};

use erebus_ipc::config::{APP_ID_ENV, write_atomic};

use super::{
    desktop_entry::DesktopEntry,
    installed::Dirs,
    vulkan::{self, ENABLE_ENV, INSTALL_DIR},
};

const CONTEXT: &str = "Context";
const ENVIRONMENT: &str = "Environment";
const FILESYSTEMS_KEY: &str = "filesystems";

/// The sandbox needs read access to the config, themes and installed layer. It also needs the
/// IPC directory; create that even if no frontend is running yet so Flatpak can share it at app
/// startup.
pub fn filesystems() -> [String; 3] {
    [
        "xdg-config/erebus:ro".to_owned(),
        "xdg-run/erebus:create".to_owned(),
        format!("xdg-data/{INSTALL_DIR}:ro"),
    ]
}

/// Recognize a Flatpak launch from its desktop entry.
pub fn app_of(entry: &DesktopEntry, exec: Option<&[String]>) -> Option<String> {
    if let Some(app) = entry
        .string(super::desktop_entry::MAIN_GROUP, "X-Flatpak")
        .filter(|app| !app.is_empty())
    {
        return Some(app);
    }

    // `flatpak run [--option=value ...] <app> [args ...]`
    let mut args = exec?.iter();
    args.find(|arg| arg.rsplit('/').next() == Some("flatpak"))?;
    if args.next()? != "run" {
        return None;
    }
    args.find(|arg| !arg.starts_with('-')).cloned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Uninjected,
    Injected,
    /// injected, but something's missing or different, e.g. the manifest was deleted
    Outdated,
}

fn override_path(dirs: &Dirs, app: &str) -> PathBuf {
    dirs.data_home.join("flatpak/overrides").join(app)
}

fn manifest_path(dirs: &Dirs, app: &str) -> PathBuf {
    dirs.home
        .join(".var/app")
        .join(app)
        .join("data/vulkan/implicit_layer.d/erebus.json")
}

fn read_override(dirs: &Dirs, app: &str) -> io::Result<DesktopEntry> {
    match fs::read_to_string(override_path(dirs, app)) {
        Ok(text) => Ok(DesktopEntry::parse(&text)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(DesktopEntry::parse("")),
        Err(err) => Err(err),
    }
}

fn filesystem_list(entry: &DesktopEntry) -> Vec<String> {
    entry
        .string(CONTEXT, FILESYSTEMS_KEY)
        .unwrap_or_default()
        .split(';')
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

fn set_filesystem_list(entry: &mut DesktopEntry, list: &[String]) {
    if list.is_empty() {
        entry.remove(CONTEXT, FILESYSTEMS_KEY);
    } else {
        entry.ensure_group(CONTEXT);
        // the way Flatpak writes lists, with a trailing separator
        entry.set_string(CONTEXT, FILESYSTEMS_KEY, &format!("{};", list.join(";")));
    }
}

fn expected_manifest(dirs: &Dirs) -> io::Result<String> {
    vulkan::manifest(&vulkan::library_in(&dirs.data_home))
}

pub fn state(dirs: &Dirs, app: &str, app_id: &str) -> State {
    let Ok(entry) = read_override(dirs, app) else {
        return State::Uninjected;
    };
    if entry.string(ENVIRONMENT, ENABLE_ENV).as_deref() != Some("1") {
        return State::Uninjected;
    }

    let list = filesystem_list(&entry);
    let complete = entry.string(ENVIRONMENT, APP_ID_ENV).as_deref() == Some(app_id)
        && filesystems().iter().all(|wanted| list.contains(wanted))
        && expected_manifest(dirs).is_ok_and(|expected| {
            fs::read_to_string(manifest_path(dirs, app)).is_ok_and(|manifest| manifest == expected)
        });

    if complete {
        State::Injected
    } else {
        State::Outdated
    }
}

/// Safe to call again for an injected app; this repairs missing override settings or manifests.
pub fn inject(dirs: &Dirs, app: &str, app_id: &str) -> io::Result<()> {
    write_atomic(
        &manifest_path(dirs, app),
        expected_manifest(dirs)?.as_bytes(),
    )?;

    let mut entry = read_override(dirs, app)?;

    let mut list = filesystem_list(&entry);
    for wanted in filesystems() {
        if !list.contains(&wanted) {
            list.push(wanted);
        }
    }
    set_filesystem_list(&mut entry, &list);

    entry.ensure_group(ENVIRONMENT);
    entry.set_string(ENVIRONMENT, ENABLE_ENV, "1");
    entry.set_string(ENVIRONMENT, APP_ID_ENV, app_id);

    write_atomic(&override_path(dirs, app), entry.to_string().as_bytes())
}

/// Takes out everything [`enable`] added, leaving the rest of the override alone.
pub fn uninject(dirs: &Dirs, app: &str) -> io::Result<()> {
    let path = override_path(dirs, app);
    let mut entry = read_override(dirs, app)?;

    entry.remove(ENVIRONMENT, ENABLE_ENV);
    entry.remove(ENVIRONMENT, APP_ID_ENV);

    let ours = filesystems();
    let list: Vec<_> = filesystem_list(&entry)
        .into_iter()
        .filter(|item| !ours.contains(item))
        .collect();
    set_filesystem_list(&mut entry, &list);
    entry.remove_if_empty(ENVIRONMENT);
    entry.remove_if_empty(CONTEXT);

    if entry.has_keys() {
        write_atomic(&path, entry.to_string().as_bytes())?;
    } else {
        remove_if_present(&path)?;
    }

    remove_if_present(&manifest_path(dirs, app))
}

fn remove_if_present(path: &std::path::Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}
