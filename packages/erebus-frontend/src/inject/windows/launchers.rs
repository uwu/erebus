//! We find Start menu shortcuts in the user's menu first, then the shared menu. The
//! first shortcut for an ID wins. IDs are relative paths such as `Zed\Zed.lnk`.
//!
//! Injection copies [`LIBRARY`] and [`ORIGINAL`] beside the app's executable. We also
//! write a [`MARKER`] containing the app ID and the installed backend's stamp. The
//! backend reads its ID here because we don't control the app's launch environment.
//! The stamp lets us recognize our copies and tell when they need updating, without
//! mistaking a DLL shipped by the app for one of ours.
//!
//! Some apps launch through `Update.exe` and install each version in an `app-<version>`
//! folder. Keep their marker in the parent folder so an update doesn't make us forget
//! that the app was injected. The new version will need another copy of the backend.
//!
//! For apps without shortcuts, the user can pick an [`executable`]. We remember these
//! paths in [`Dirs::added`] so later scans can find them. Their IDs are executable
//! paths; shortcut IDs have the `.lnk` removed when used in the config.

use std::{
    cmp::Ordering,
    collections::HashSet,
    fmt, fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use erebus_ipc::config::{APP_ID_MARKER, write_atomic};

use crate::library::LaunchedApp;

use super::{
    d3d11::{self, LIBRARY, ORIGINAL},
    icons,
    shortcut::{self, Shortcut},
};

/// Stores the app ID and backend stamp beside the injected library.
pub const MARKER: &str = APP_ID_MARKER;
/// Suffix for files awaiting deletion because an app still has them loaded.
const RETIRED: &str = "erebus-old";

/// how far below a Start menu folder shortcuts are looked for
const MAX_DEPTH: usize = 4;

#[derive(Debug, Clone)]
pub struct Dirs {
    /// the Start menu's `Programs` folders, the user's own first
    pub start_menus: Vec<PathBuf>,
    /// where icons taken out of apps are kept
    pub icons: PathBuf,
    /// the executables set up without a shortcut, one per line
    pub added: PathBuf,
}

impl Dirs {
    pub fn from_env() -> io::Result<Self> {
        const PROGRAMS: &str = r"Microsoft\Windows\Start Menu\Programs";

        let roaming = dirs::data_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no data directory"))?;
        let shared = std::env::var_os("ProgramData").map(PathBuf::from);

        let icons = dirs::cache_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no cache directory"))?
            .join(r"erebus\icons");
        let added = dirs::data_local_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no local data directory"))?
            .join(r"erebus\executables.txt");

        Ok(Self {
            start_menus: std::iter::once(roaming)
                .chain(shared)
                .map(|dir| dir.join(PROGRAMS))
                .collect(),
            icons,
            added,
        })
    }
}

/// An app that can be launched from the Start menu, or one picked by hand.
#[derive(Debug, Clone)]
pub struct Launcher {
    /// the shortcut's path relative to its Start menu, like `Zed\Zed.lnk`, or the
    /// executable's path if it has no shortcut
    pub id: String,
    /// the shortcut that's in effect for the ID, or the executable
    pub path: PathBuf,
    pub name: String,
    /// the path of a PNG taken from the app's icon
    pub icon: Option<String>,
    /// Executable filename.
    pub program: Option<String>,
    pub target: Option<Target>,
    pub kind: Kind,
    pub state: State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Shortcut,
    /// picked by hand, and in [`Dirs::added`] once it's set up
    Executable,
}

#[derive(Debug, Clone)]
pub struct Target {
    /// Copy [`LIBRARY`] beside this executable.
    pub executable: PathBuf,
    /// Keep [`MARKER`] here. For versioned installs this is the parent of the version folders;
    /// otherwise it's the executable's folder.
    pub root: PathBuf,
}

impl Target {
    fn dir(&self) -> Option<&Path> {
        self.executable.parent()
    }

    /// Whether each version of the app has a folder of its own in `root`.
    fn is_versioned(&self) -> bool {
        self.dir() != Some(&self.root)
    }
}

impl Launcher {
    /// The ID the app is configured under once it's set up.
    pub fn app_id(&self) -> String {
        app_id(&self.id)
    }

    /// The app as the library knows it once it's set up.
    pub fn launched_app(&self) -> LaunchedApp {
        LaunchedApp {
            id: self.app_id(),
            launcher: self.id.clone(),
            name: self.name.clone(),
            icon: self.icon.clone(),
        }
    }

    /// Show the executable's location beneath the app name.
    pub fn detail(&self) -> String {
        match &self.target {
            Some(target) => target.executable.display().to_string(),
            None => self.id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Off,
    On,
    /// on, but needs changing again: the frontend shipped a different library, the app
    /// updated itself into a new folder, or it was set up under another ID
    Outdated,
    /// on, but the executable is gone, e.g. because the app was uninstalled
    Orphaned,
    Unsupported(Unsupported),
}

impl State {
    /// Whether launching the app loads erebus.
    pub fn is_on(&self) -> bool {
        matches!(self, Self::On | Self::Outdated | Self::Orphaned)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// Windows Installer resolves the executable at launch.
    Advertised,
    OwnLibrary,
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Advertised => {
                "This shortcut is managed by Windows Installer, which erebus can't inject into"
            }
            Self::OwnLibrary => "This app comes with its own d3d11.dll",
        })
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Unsupported(Unsupported),
    /// the app is already in the state that was asked for
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

/// Include manually added executables alongside Start menu apps. Sort the combined list by
/// name.
pub fn scan(dirs: &Dirs) -> Vec<Launcher> {
    let mut seen = HashSet::new();
    let mut executables = HashSet::new();
    let mut launchers = Vec::new();

    // Check manually added executables first. If one has gained a shortcut since setup, it
    // should keep its existing ID.
    for path in added(dirs) {
        if !executables.insert(key(&path)) {
            continue;
        }
        // Skip saved executables whose injection has since been removed.
        if let Some(launcher) = read_executable(dirs, path)
            && launcher.state.is_on()
        {
            launchers.push(launcher);
        }
    }

    for dir in &dirs.start_menus {
        let mut found = Vec::new();
        collect(dir, dir, 0, &mut found);
        // Filesystem iteration order isn't stable, so sort before resolving duplicates.
        found.sort();

        for (id, path) in found {
            // A higher-priority Start menu already supplied this ID.
            if !seen.insert(id.to_lowercase()) {
                continue;
            }

            let Some(launcher) = read_launcher(dirs, id, path) else {
                continue;
            };
            // more than one shortcut to the same app, like one in each Start menu
            if let Some(target) = &launcher.target
                && !executables.insert(key(&target.executable))
            {
                continue;
            }
            launchers.push(launcher);
        }
    }

    launchers.sort_by(|a, b| compare_names(&a.name, &b.name).then_with(|| a.id.cmp(&b.id)));
    launchers
}

/// The app ID for a launcher ID.
pub fn app_id(launcher_id: &str) -> String {
    let path = Path::new(launcher_id);
    if has_extension(path, "lnk") {
        path.with_extension("").to_string_lossy().into_owned()
    } else {
        launcher_id.to_owned()
    }
}

fn compare_names(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// Ignore case when comparing Windows paths.
fn key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
}

/// Collects `(id, path)` for every `.lnk` file under `dir`.
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

        if !has_extension(&path, "lnk") {
            continue;
        }

        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        found.push((relative.to_string_lossy().into_owned(), path));
    }
}

/// `None` for shortcuts that don't start an app.
fn read_launcher(dirs: &Dirs, id: String, path: PathBuf) -> Option<Launcher> {
    let shortcut = shortcut::read(&path)?;
    let name = path.file_stem()?.to_string_lossy().into_owned();

    let file_name = |path: &Path| Some(path.file_name()?.to_string_lossy().into_owned());
    let is_uninstaller = name.to_lowercase().contains("uninstall")
        || shortcut
            .target
            .as_deref()
            .and_then(file_name)
            .is_some_and(|file| file.to_lowercase().starts_with("unins"));
    if is_uninstaller {
        return None;
    }

    let target = match shortcut.target.as_deref() {
        Some(target) if has_extension(target, "exe") => Some(resolve(&shortcut, target)),
        None if shortcut.advertised => None,
        _ => return None,
    };

    let state = match &target {
        Some(target) => state(target, &app_id(&id))?,
        None => State::Unsupported(Unsupported::Advertised),
    };

    let icon = match (&shortcut.icon_location, &target) {
        (Some(location), _) if location.is_file() => icon(dirs, &id, location, shortcut.icon_index),
        (_, Some(target)) => icon(dirs, &id, &target.executable, 0),
        _ => None,
    };

    Some(Launcher {
        program: target
            .as_ref()
            .and_then(|target| file_name(&target.executable)),
        icon,
        name,
        id,
        path,
        target,
        kind: Kind::Shortcut,
        state,
    })
}

/// Prefer an existing shortcut for the chosen executable so picking it manually doesn't create
/// a second injection.
pub fn executable(dirs: &Dirs, path: &Path) -> Result<Launcher> {
    if !has_extension(path, "exe") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} isn't an executable", path.display()),
        )
        .into());
    }

    let found = scan(dirs).into_iter().find(|launcher| {
        launcher
            .target
            .as_ref()
            .is_some_and(|target| key(&target.executable) == key(path))
    });
    if let Some(launcher) = found {
        return Ok(launcher);
    }

    read_executable(dirs, path.to_owned()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} doesn't exist", path.display()),
        )
        .into()
    })
}

/// `None` for an executable that isn't there anymore and was never set up.
fn read_executable(dirs: &Dirs, path: PathBuf) -> Option<Launcher> {
    let target = Target {
        root: path.parent()?.to_owned(),
        executable: path.clone(),
    };
    let id = path.to_string_lossy().into_owned();

    Some(Launcher {
        name: path.file_stem()?.to_string_lossy().into_owned(),
        icon: icon(dirs, &id, &path, 0),
        program: Some(path.file_name()?.to_string_lossy().into_owned()),
        state: state(&target, &app_id(&id))?,
        target: Some(target),
        kind: Kind::Executable,
        path,
        id,
    })
}

fn added(dirs: &Dirs) -> Vec<PathBuf> {
    fs::read_to_string(&dirs.added)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn set_added(dirs: &Dirs, paths: &[PathBuf]) -> io::Result<()> {
    let text: String = paths
        .iter()
        .map(|path| format!("{}\n", path.display()))
        .collect();
    write_atomic(&dirs.added, text.as_bytes())
}

/// Looks past an `Update.exe --processStart <app>.exe` shortcut to the newest version of
/// the app it starts.
fn resolve(shortcut: &Shortcut, target: &Path) -> Target {
    let plain = || Target {
        executable: target.to_owned(),
        root: target.parent().unwrap_or(target).to_owned(),
    };

    let is_updater = target
        .file_name()
        .is_some_and(|file| file.eq_ignore_ascii_case("Update.exe"));
    let args = shortcut::split_arguments(&shortcut.arguments);
    let program = args
        .iter()
        .position(|arg| arg.starts_with("--processStart"))
        .and_then(|index| args.get(index + 1));
    let (true, Some(program), Some(root)) = (is_updater, program, target.parent()) else {
        return plain();
    };

    let newest = versions(root)
        .into_iter()
        .map(|dir| dir.join(program))
        .find(|executable| executable.is_file());

    Target {
        // the app was uninstalled, but this is still where it'd be
        executable: newest.unwrap_or_else(|| root.join(program)),
        root: root.to_owned(),
    }
}

/// Folders called `app-<version>` in `root`, newest first.
fn versions(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut versions: Vec<(Vec<u64>, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let version = name.to_str()?.strip_prefix("app-")?;
            let parts = version.split('.').map(str::parse).collect::<Result<_, _>>();
            Some((parts.ok()?, entry.path()))
        })
        .filter(|(_, path)| path.is_dir())
        .collect();

    versions.sort_by(|a, b| b.0.cmp(&a.0));
    versions.into_iter().map(|(_, path)| path).collect()
}

/// `None` for an app that isn't there anymore and was never set up.
fn state(target: &Target, app_id: &str) -> Option<State> {
    let marker = fs::read_to_string(target.root.join(MARKER)).ok();

    if !target.executable.is_file() {
        return marker.map(|_| State::Orphaned);
    }
    let dir = target.dir()?;

    let Some(marker) = marker else {
        if dir.join(LIBRARY).exists() {
            return Some(State::Unsupported(Unsupported::OwnLibrary));
        }
        return Some(State::Off);
    };

    let complete = d3d11::installed_stamp().is_some_and(|stamp| marker == contents(app_id, &stamp))
        && dir.join(LIBRARY).is_file()
        && dir.join(ORIGINAL).is_file();
    Some(if complete { State::On } else { State::Outdated })
}

/// Cache the extracted PNG until the source file changes.
fn icon(dirs: &Dirs, id: &str, source: &Path, index: i32) -> Option<String> {
    let file: String = id
        .chars()
        .map(|char| if char.is_alphanumeric() { char } else { '_' })
        .collect();
    let cached = dirs.icons.join(file).with_extension("png");

    let modified = |path: &Path| fs::metadata(path).and_then(|meta| meta.modified()).ok();
    let fresh = match (modified(&cached), modified(source)) {
        (Some(cached), Some(source)) => cached >= source,
        (cached, None) => cached.is_some(),
        (None, _) => false,
    };

    if !fresh {
        let png = icons::png(source, index).ok().flatten()?;
        write_atomic(&cached, &png).ok()?;
    }

    Some(cached.to_string_lossy().into_owned())
}

//
// MARK: changing apps
//

/// Makes the app launch with erebus.
pub fn enable(dirs: &Dirs, launcher: &Launcher) -> Result<()> {
    match launcher.state {
        State::Off => {}
        State::Unsupported(reason) => return Err(Error::Unsupported(reason)),
        State::On | State::Outdated | State::Orphaned => return Err(Error::Unchanged),
    }

    // Save the path before injecting. A failed injection is harmless here because scans skip
    // entries that aren't injected.
    if launcher.kind == Kind::Executable {
        let mut list = added(dirs);
        if !list.iter().any(|path| key(path) == key(&launcher.path)) {
            list.push(launcher.path.clone());
            set_added(dirs, &list)?;
        }
    }

    copy_into(target_of(launcher)?, &launcher.app_id())
}

/// Puts the app back the way it launched before erebus.
pub fn disable(dirs: &Dirs, launcher: &Launcher) -> Result<()> {
    if !launcher.state.is_on() {
        return Err(Error::Unchanged);
    }

    let target = target_of(launcher)?;
    let folders: Vec<PathBuf> = if target.is_versioned() {
        versions(&target.root)
    } else {
        target.dir().map(Path::to_owned).into_iter().collect()
    };

    for dir in folders {
        for file in [LIBRARY, ORIGINAL] {
            retire(&dir.join(file)).map_err(|err| protected(&dir, err))?;
        }
        sweep(&dir);
    }

    match fs::remove_file(target.root.join(MARKER)) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err.into()),
        _ => {}
    }

    if launcher.kind == Kind::Executable {
        let mut list = added(dirs);
        list.retain(|path| key(path) != key(&launcher.path));
        set_added(dirs, &list)?;
    }
    Ok(())
}

/// Copies the installed library to an [`Outdated`](State::Outdated) app again.
pub fn update(_dirs: &Dirs, launcher: &Launcher) -> Result<()> {
    if launcher.state != State::Outdated {
        return Err(Error::Unchanged);
    }

    copy_into(target_of(launcher)?, &launcher.app_id())
}

/// Refresh outdated injections. Return the names of updated apps and an error message for each
/// failure.
pub fn update_all(dirs: &Dirs, launchers: &[Launcher]) -> (Vec<String>, Vec<String>) {
    let mut updated = Vec::new();
    let mut problems = Vec::new();

    for launcher in launchers.iter().filter(|l| l.state == State::Outdated) {
        match update(dirs, launcher) {
            Ok(()) => updated.push(launcher.name.clone()),
            Err(err) => problems.push(format!("Couldn't update {}: {err}", launcher.name)),
        }
    }

    (updated, problems)
}

fn target_of(launcher: &Launcher) -> Result<&Target> {
    launcher
        .target
        .as_ref()
        .ok_or(Error::Unsupported(Unsupported::Advertised))
}

fn contents(app_id: &str, stamp: &str) -> String {
    format!("{app_id}\n{stamp}")
}

fn copy_into(target: &Target, app_id: &str) -> Result<()> {
    let dir = target
        .dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the app has no folder"))?;
    let stamp = d3d11::installed_stamp().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("the {} isn't installed", d3d11::NAME),
        )
    })?;
    let library = fs::read(d3d11::installed_library()?)?;
    let original = fs::read(d3d11::system_library())?;

    sweep(dir);
    // Write the marker before the DLLs. If a later write fails, the next scan can recognize the
    // partial injection and offer to repair it.
    write_atomic(
        &target.root.join(MARKER),
        contents(app_id, &stamp).as_bytes(),
    )
    .and_then(|()| replace(&dir.join(ORIGINAL), &original))
    .and_then(|()| replace(&dir.join(LIBRARY), &library))
    .map_err(|err| protected(dir, err).into())
}

fn protected(dir: &Path, err: io::Error) -> io::Error {
    if err.kind() != io::ErrorKind::PermissionDenied {
        return err;
    }

    io::Error::new(
        err.kind(),
        format!(
            "{} can only be changed by an administrator, or a file in it is in use",
            dir.display()
        ),
    )
}

/// Windows lets us rename a loaded DLL but won't let us replace it. Move the old copy aside
/// before writing the new one.
fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    retire(path)?;
    write_atomic(path, bytes)
}

/// Try deleting the file now. If an app still has it loaded, rename it for later cleanup.
fn retire(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let mut aside = path.as_os_str().to_owned();
            aside.push(format!(".{nanos}.{RETIRED}"));
            fs::rename(path, aside)
        }
        result => result,
    }
}

/// Retry deleting files left behind by [`retire`]. Some may still be loaded.
fn sweep(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if has_extension(&path, RETIRED) {
            let _ = fs::remove_file(path);
        }
    }
}
