//! We inject ordinary Linux apps by editing their desktop entries. The user's entries
//! take precedence over system entries, so we can copy a system entry into
//! `$XDG_DATA_HOME/applications` without changing the original. Entries already owned
//! by the user are edited in place. Scanning follows the same precedence: the first ID
//! found in `$XDG_DATA_HOME`, then `$XDG_DATA_DIRS`, wins.
//!
//! Each `Exec` line gets an `env EREBUS_ENABLE=1 EREBUS_APP=<id>` prefix, including
//! actions like "New Window". The ID is the desktop file ID without `.desktop`
//! (see [`InstalledApp::app_id`]), so the backend uses the right config even when the
//! entry launches another process.
//!
//! Keep the original command in [`ORIGINAL_EXEC`] so removal can restore it. For copied
//! entries, [`SOURCE`] records where the copy came from; we use that to detect app
//! updates. Removing an injection deletes our copy or restores the user's original
//! commands. The entries themselves hold the injection state.
//!
//! Flatpak entries are left alone. Those apps need a sandbox override instead
//! (see [`flatpak`](super::flatpak)).

use std::{
    cmp::Ordering,
    collections::HashSet,
    env, fmt, fs, io,
    path::{Path, PathBuf},
    process::Command,
    thread,
};

use erebus_ipc::config::{APP_ID_ENV, write_atomic};

use crate::library::LibraryApp;

use super::{
    desktop_entry::{DesktopEntry, MAIN_GROUP, exec_program, quote_exec_arg, split_exec},
    flatpak,
    vulkan::ENABLE_ENV,
};

/// the `Exec` line a group had before erebus changed it
pub const ORIGINAL_EXEC: &str = "X-Erebus-Original-Exec";
/// on copies of system entries, the path of the entry that was copied
pub const SOURCE: &str = "X-Erebus-Source";

/// how far below an `applications` directory entries are looked for
const MAX_DEPTH: usize = 4;

/// The directories desktop entries come from.
#[derive(Debug, Clone)]
pub struct Dirs {
    /// `$XDG_DATA_HOME`, where entries we write go
    pub data_home: PathBuf,
    /// `$XDG_DATA_DIRS`, most important first
    pub data_dirs: Vec<PathBuf>,
    /// the home directory, which Flatpak apps keep their data under
    pub home: PathBuf,
}

impl Dirs {
    pub fn from_env() -> io::Result<Self> {
        let data_home = dirs::data_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no data directory"))?;

        let data_dirs = env::var("XDG_DATA_DIRS")
            .ok()
            .filter(|dirs| !dirs.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
        let data_dirs = env::split_paths(&data_dirs)
            .filter(|dir| dir.is_absolute())
            .collect();

        let home = dirs::home_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?;

        Ok(Self {
            data_home,
            data_dirs,
            home,
        })
    }

    pub fn user_applications(&self) -> PathBuf {
        self.data_home.join("applications")
    }

    fn applications(&self) -> impl Iterator<Item = PathBuf> + '_ {
        std::iter::once(self.user_applications())
            .chain(self.data_dirs.iter().map(|dir| dir.join("applications")))
    }
}

/// An app that can be launched from the desktop.
#[derive(Debug, Clone)]
pub struct InstalledApp {
    /// the desktop file ID, like `dev.zed.Zed.desktop`
    pub id: String,
    /// the entry that's in effect for the ID
    pub path: PathBuf,
    pub name: String,
    /// an icon name or path, as `Icon` has it
    pub icon: Option<String>,
    /// Executable filename.
    pub program: Option<String>,
    pub kind: Kind,
    pub state: State,
}

/// How an app is injected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// by changing its desktop entry
    DesktopEntry,
    /// with an override for this Flatpak app
    Flatpak { app: String },
}

impl InstalledApp {
    /// The ID the app is configured under once it's injected.
    pub fn app_id(&self) -> String {
        app_id(&self.id)
    }

    /// The app as the library knows it once it's injected.
    pub fn library_app(&self) -> LibraryApp {
        LibraryApp {
            id: self.app_id(),
            installed_app: self.id.clone(),
            name: self.name.clone(),
            icon: self.icon.clone(),
        }
    }

    /// Show the executable or Flatpak ID beneath the app name.
    pub fn detail(&self) -> String {
        match &self.kind {
            Kind::Flatpak { app } => format!("{app} (Flatpak)"),
            Kind::DesktopEntry => self.program.clone().unwrap_or_else(|| self.id.clone()),
        }
    }
}

/// The app ID for a desktop file ID.
pub fn app_id(desktop_id: &str) -> String {
    desktop_id
        .strip_suffix(".desktop")
        .unwrap_or(desktop_id)
        .to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Uninjected,
    Injected,
    /// injected, but needs changing again: the system entry was changed after we copied it (e.g.
    /// by an update), or an older version of erebus injected it
    Outdated,
    /// injected, but the system entry we copied is gone, e.g. because the app was uninstalled
    Orphaned,
    Unsupported(Unsupported),
}

impl State {
    /// Whether launching the app loads erebus, including when it needs updating.
    pub fn is_injected(&self) -> bool {
        matches!(self, Self::Injected | Self::Outdated | Self::Orphaned)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    Snap,
    DBus,
    NoExec,
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Snap => "Snap apps aren't supported",
            Self::DBus => "This app is started over D-Bus, which erebus can't inject into",
            Self::NoExec => "This entry doesn't say how to start the app",
        })
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Unsupported(Unsupported),
    /// the entry is already in the state that was asked for
    Unchanged,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => err.fmt(f),
            Self::Unsupported(reason) => reason.fmt(f),
            Self::Unchanged => f.write_str("nothing to change"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

//
// MARK: scanning
//

/// Every app with a desktop entry that's shown in app menus, sorted by name.
pub fn scan(dirs: &Dirs) -> Vec<InstalledApp> {
    let locales = locales();
    let mut seen = HashSet::new();
    let mut apps = Vec::new();

    for dir in dirs.applications() {
        let mut found = Vec::new();
        collect(&dir, &dir, 0, &mut found);
        // Filesystem iteration order isn't stable, so sort before resolving duplicates.
        found.sort();

        for (id, path) in found {
            // A higher-priority directory already supplied this ID.
            if !seen.insert(id.clone()) {
                continue;
            }

            if let Some(installed) = read_app(dirs, id, path, &locales) {
                apps.push(installed);
            }
        }
    }

    apps.sort_by(|a, b| compare_names(&a.name, &b.name).then_with(|| a.id.cmp(&b.id)));
    apps
}

fn compare_names(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// Collects `(id, path)` for every `.desktop` file under `dir`. IDs are the path relative
/// to `root` with `/` replaced by `-`.
fn collect(root: &Path, dir: &Path, depth: usize, found: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < MAX_DEPTH {
                collect(root, &path, depth + 1, found);
            }
            continue;
        }

        if path
            .extension()
            .is_none_or(|extension| extension != "desktop")
        {
            continue;
        }

        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let id = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("-");
        found.push((id, path));
    }
}

/// `None` for entries that aren't apps or aren't meant to be shown.
fn read_app(dirs: &Dirs, id: String, path: PathBuf, locales: &[String]) -> Option<InstalledApp> {
    let entry = DesktopEntry::parse(&fs::read_to_string(&path).ok()?);

    if entry.raw(MAIN_GROUP, "Type") != Some("Application")
        || entry.bool(MAIN_GROUP, "Hidden")
        || entry.bool(MAIN_GROUP, "NoDisplay")
    {
        return None;
    }

    let exec = entry.string(MAIN_GROUP, "Exec");
    let program = exec.as_deref().and_then(exec_program);
    let args = exec.as_deref().and_then(split_exec);

    let (kind, state) = match flatpak::app_of(&entry, args.as_deref()) {
        Some(app) => {
            let state = match flatpak::state(dirs, &app, &app_id(&id)) {
                flatpak::State::Uninjected => State::Uninjected,
                flatpak::State::Injected => State::Injected,
                flatpak::State::Outdated => State::Outdated,
            };
            (Kind::Flatpak { app }, state)
        }
        None => (
            Kind::DesktopEntry,
            state(&entry, &path, &app_id(&id), program.as_deref()),
        ),
    };

    Some(InstalledApp {
        name: entry
            .localized(MAIN_GROUP, "Name", locales)
            .unwrap_or_else(|| id.trim_end_matches(".desktop").to_owned()),
        icon: entry
            .localized(MAIN_GROUP, "Icon", locales)
            .filter(|icon| !icon.is_empty()),
        program: program.map(|program| program.rsplit('/').next().unwrap_or(&program).to_owned()),
        id,
        path,
        kind,
        state,
    })
}

fn state(entry: &DesktopEntry, path: &Path, app_id: &str, program: Option<&str>) -> State {
    if exec_groups(entry).any(|group| entry.raw(&group, ORIGINAL_EXEC).is_some()) {
        let source = entry.string(MAIN_GROUP, SOURCE).map(PathBuf::from);
        let modified = |path: &Path| fs::metadata(path).and_then(|meta| meta.modified());

        let source_changed = match source.as_deref().map(modified) {
            None => false,
            Some(Err(err)) if err.kind() == io::ErrorKind::NotFound => return State::Orphaned,
            Some(source) => matches!(
                (source, modified(path)),
                (Ok(source), Ok(copy)) if source > copy
            ),
        };

        let exec_changed = exec_groups(entry).any(|group| {
            let original = entry.raw(&group, ORIGINAL_EXEC);
            original
                .map(|original| patched_exec(original, app_id))
                .as_deref()
                != entry.raw(&group, "Exec")
        });

        return if source_changed || exec_changed {
            State::Outdated
        } else {
            State::Injected
        };
    }

    match unsupported(entry, program) {
        Some(reason) => State::Unsupported(reason),
        None => State::Uninjected,
    }
}

fn unsupported(entry: &DesktopEntry, program: Option<&str>) -> Option<Unsupported> {
    let Some(program) = program else {
        return Some(Unsupported::NoExec);
    };

    if entry.raw(MAIN_GROUP, "X-SnapInstanceName").is_some()
        || program.starts_with("/snap/")
        || program.rsplit('/').next() == Some("snap")
    {
        return Some(Unsupported::Snap);
    }
    if entry.bool(MAIN_GROUP, "DBusActivatable") {
        return Some(Unsupported::DBus);
    }

    None
}

/// The groups that have an `Exec` line: the main one, and actions.
fn exec_groups(entry: &DesktopEntry) -> impl Iterator<Item = String> + '_ {
    entry
        .group_names()
        .into_iter()
        .filter(|group| group == MAIN_GROUP || group.starts_with("Desktop Action "))
        .filter(|group| entry.raw(group, "Exec").is_some())
}

/// Locales to look for translations in, most specific first, from the environment.
fn locales() -> Vec<String> {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(|var| env::var(var).ok())
        .find(|value| !value.is_empty());

    let Some(locale) = locale else {
        return Vec::new();
    };

    // lang_COUNTRY.ENCODING@MODIFIER; translations don't use the encoding
    let (locale, modifier) = match locale.split_once('@') {
        Some((locale, modifier)) => (locale, Some(modifier)),
        None => (locale.as_str(), None),
    };
    let locale = locale.split('.').next().unwrap_or(locale);
    let lang = locale.split('_').next().unwrap_or(locale);

    let mut locales = Vec::new();
    for base in [locale, lang] {
        if let Some(modifier) = modifier {
            locales.push(format!("{base}@{modifier}"));
        }
        locales.push(base.to_owned());
    }
    locales.dedup();
    locales.retain(|locale| locale != "C" && locale != "POSIX");
    locales
}

//
// MARK: changing entries
//

/// The injection takes effect when the app next starts.
pub fn inject(dirs: &Dirs, installed: &InstalledApp) -> Result<()> {
    match installed.state {
        State::Uninjected => {}
        State::Unsupported(reason) => return Err(Error::Unsupported(reason)),
        State::Injected | State::Outdated | State::Orphaned => return Err(Error::Unchanged),
    }

    if let Kind::Flatpak { app } = &installed.kind {
        return Ok(flatpak::inject(dirs, app, &installed.app_id())?);
    }

    let text = fs::read_to_string(&installed.path)?;
    let mut entry = DesktopEntry::parse(&text);
    patch(&mut entry, &installed.app_id());

    if is_user_entry(dirs, &installed.path) {
        write_entry(&installed.path, &entry)?;
    } else {
        let source = installed.path.to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "entry path isn't valid UTF-8")
        })?;
        entry.set_string(MAIN_GROUP, SOURCE, source);
        write_entry(&dirs.user_applications().join(&installed.id), &entry)?;
    }

    refresh_caches(dirs);
    Ok(())
}

/// Restore the app's original launch setup.
pub fn uninject(dirs: &Dirs, installed: &InstalledApp) -> Result<()> {
    if !installed.state.is_injected() {
        return Err(Error::Unchanged);
    }

    if let Kind::Flatpak { app } = &installed.kind {
        return Ok(flatpak::uninject(dirs, app)?);
    }

    let text = fs::read_to_string(&installed.path)?;
    let mut entry = DesktopEntry::parse(&text);

    if entry.raw(MAIN_GROUP, SOURCE).is_some() {
        // our own copy, so the system entry takes over again once it's gone
        match fs::remove_file(&installed.path) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err.into()),
            _ => {}
        }
    } else {
        unpatch(&mut entry);
        write_entry(&installed.path, &entry)?;
    }

    refresh_caches(dirs);
    Ok(())
}

/// Rebuild outdated copies from the current system entry so we pick up changes from app
/// updates.
pub fn update(dirs: &Dirs, installed: &InstalledApp) -> Result<()> {
    if installed.state != State::Outdated {
        return Err(Error::Unchanged);
    }

    if let Kind::Flatpak { app } = &installed.kind {
        return Ok(flatpak::inject(dirs, app, &installed.app_id())?);
    }

    let mut entry = DesktopEntry::parse(&fs::read_to_string(&installed.path)?);
    let app_id = installed.app_id();

    match entry.string(MAIN_GROUP, SOURCE).map(PathBuf::from) {
        Some(source) => {
            entry = DesktopEntry::parse(&fs::read_to_string(&source)?);
            patch(&mut entry, &app_id);
            entry.set_string(MAIN_GROUP, SOURCE, &source.to_string_lossy());
        }
        None => {
            unpatch(&mut entry);
            patch(&mut entry, &app_id);
        }
    }
    write_entry(&installed.path, &entry)?;

    refresh_caches(dirs);
    Ok(())
}

/// Refresh outdated injections. Return the names of updated apps and an error message for each
/// failure.
pub fn update_all(dirs: &Dirs, apps: &[InstalledApp]) -> (Vec<String>, Vec<String>) {
    let mut updated = Vec::new();
    let mut problems = Vec::new();

    for installed in apps.iter().filter(|l| l.state == State::Outdated) {
        match update(dirs, installed) {
            Ok(()) => updated.push(installed.name.clone()),
            Err(err) => problems.push(format!("Couldn't update {}: {err}", installed.name)),
        }
    }

    (updated, problems)
}

/// The `Exec` line that starts `original` with erebus, as `app_id`.
fn patched_exec(original: &str, app_id: &str) -> String {
    let app = quote_exec_arg(&format!("{APP_ID_ENV}={app_id}"));
    format!("env {ENABLE_ENV}=1 {app} {original}")
}

fn patch(entry: &mut DesktopEntry, app_id: &str) {
    for group in exec_groups(entry).collect::<Vec<_>>() {
        if entry.raw(&group, ORIGINAL_EXEC).is_some() {
            continue;
        }

        let exec = entry.raw(&group, "Exec").unwrap_or_default().to_owned();
        entry.set_raw(&group, ORIGINAL_EXEC, &exec);
        entry.set_raw(&group, "Exec", &patched_exec(&exec, app_id));
    }
}

fn unpatch(entry: &mut DesktopEntry) {
    for group in entry.group_names() {
        if let Some(exec) = entry.raw(&group, ORIGINAL_EXEC).map(str::to_owned) {
            entry.set_raw(&group, "Exec", &exec);
            entry.remove(&group, ORIGINAL_EXEC);
        }
    }
}

fn is_user_entry(dirs: &Dirs, path: &Path) -> bool {
    path.starts_with(dirs.user_applications())
}

fn write_entry(path: &Path, entry: &DesktopEntry) -> io::Result<()> {
    write_atomic(path, entry.to_string().as_bytes())
}

/// Desktop environments notice new entries by themselves, but the MIME cache used for
/// "Open With" needs rebuilding.
fn refresh_caches(dirs: &Dirs) {
    let dir = dirs.user_applications();
    thread::spawn(move || {
        // it's fine for this to be missing
        let _ = Command::new("update-desktop-database").arg(dir).output();
    });
}
