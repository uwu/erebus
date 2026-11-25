//! State tracking for themes and apps.

use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use erebus_ipc::config::{self, AppTheme, ConfigStore, ThemeEntry};
use erebus_theme::{OwnedManifest, settings::OwnedSettingsField};

#[derive(Default)]
pub struct Library {
    pub config_store: ConfigStore,
    /// keyed by theme ID
    pub manifests: HashMap<String, Result<OwnedManifest, String>>,
}

impl Library {
    pub fn load() -> io::Result<Self> {
        let store = ConfigStore::load()?;
        let manifests = store
            .themes
            .iter()
            .map(|entry| (entry.id.clone(), load_manifest(entry)))
            .collect();

        Ok(Self {
            config_store: store,
            manifests,
        })
    }

    pub fn theme_info<'a>(&'a self, entry: &'a ThemeEntry) -> ThemeInfo<'a> {
        ThemeInfo::new(entry, self.manifests.get(&entry.id))
    }

    fn position(&self, id: &str) -> Option<usize> {
        self.config_store
            .themes
            .iter()
            .position(|entry| entry.id == id)
    }

    //
    // MARK: apps
    //

    /// Adds `process` to the known apps. Returns whether it wasn't known before.
    pub fn register_app(&mut self, process: &str) -> bool {
        if self.config_store.apps.contains_key(process) {
            return false;
        }

        self.config_store
            .apps
            .insert(process.to_owned(), Default::default());
        true
    }

    /// Adds or updates an app a frontend injected, keeping its theme settings
    /// if it's known already. Returns whether anything changed.
    pub fn set_app(&mut self, app: &LibraryApp) -> bool {
        let config = self.config_store.apps.entry(app.id.clone()).or_default();
        let before = (
            config.installed_app.clone(),
            config.name.clone(),
            config.icon.clone(),
        );

        config.installed_app = Some(app.installed_app.clone());
        config.name = Some(app.name.clone());
        config.icon = app.icon.clone();

        before
            != (
                config.installed_app.clone(),
                config.name.clone(),
                config.icon.clone(),
            )
    }

    /// Makes the apps a frontend injected match `injected`, every installed app that's
    /// injected right now: missing ones are added, and ones that were uninjected some other
    /// way are dropped. Returns whether anything changed.
    pub fn sync_apps(&mut self, injected: &[LibraryApp]) -> bool {
        let before = self.config_store.apps.len();
        self.config_store.apps.retain(|id, config| {
            config.installed_app.as_ref().is_none_or(|installed| {
                injected
                    .iter()
                    .any(|app| app.id == *id && app.installed_app == *installed)
            })
        });
        let mut changed = self.config_store.apps.len() != before;

        for app in injected {
            changed |= self.set_app(app);
        }
        changed
    }

    /// The installed app `process` was injected through, if a frontend injected it.
    pub fn installed_app(&self, process: &str) -> Option<&str> {
        self.config_store
            .apps
            .get(process)?
            .installed_app
            .as_deref()
    }

    /// What to call an app.
    pub fn app_name<'a>(&'a self, process: &'a str) -> &'a str {
        self.config_store
            .apps
            .get(process)
            .and_then(|app| app.name.as_deref())
            .unwrap_or(process)
    }

    pub fn app_icon(&self, process: &str) -> Option<&str> {
        self.config_store.apps.get(process)?.icon.as_deref()
    }

    pub fn app_theme(&self, process: &str, id: &str) -> Option<&AppTheme> {
        self.config_store.apps.get(process)?.themes.get(id)
    }

    pub fn app_theme_mut(&mut self, process: &str, id: &str) -> &mut AppTheme {
        let app = self
            .config_store
            .apps
            .entry(process.to_owned())
            .or_default();
        app.themes.entry(id.to_owned()).or_default()
    }

    //
    // MARK: themes
    //

    /// Installing the same manifest ID updates its binary and source without resetting settings.
    pub fn install(&mut self, source: &Path) -> io::Result<()> {
        let source = source.canonicalize()?;
        let wasm = fs::read(&source)?;
        let manifest = read_manifest(&wasm)?;
        self.install_bytes(source, &wasm, manifest, &config::themes_dir()?)
    }

    fn install_bytes(
        &mut self,
        source: PathBuf,
        wasm: &[u8],
        manifest: OwnedManifest,
        dir: &Path,
    ) -> io::Result<()> {
        let id = manifest.id.clone();
        let same_source = |entry: &ThemeEntry| {
            entry.source == source || entry.source.canonicalize().is_ok_and(|path| path == source)
        };
        if self
            .config_store
            .themes
            .iter()
            .any(|entry| entry.id != id && same_source(entry))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "theme ID changed at an installed source; keep its original ID or import the new theme from a different path",
            ));
        }
        let entry = ThemeEntry {
            id: id.clone(),
            source,
        };
        // Don't change the library or preferences until the new binary is safely on disk.
        config::write_atomic(&entry.binary_path_in(dir)?, wasm)?;

        if let Some(index) = self.position(&id) {
            self.config_store.themes[index] = entry;
        } else {
            self.config_store.themes.push(entry);
        }
        self.manifests.insert(id, Ok(manifest));
        Ok(())
    }

    /// Installs every `.wasm` file in `paths`. Returns whether any were installed, and a
    /// message for each one that wasn't.
    pub fn install_files(&mut self, paths: &[PathBuf]) -> (bool, Vec<String>) {
        let mut installed = false;
        let mut problems = Vec::new();

        for path in paths {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if path.extension().is_none_or(|extension| extension != "wasm") {
                problems.push(format!("Skipped {name}, which isn't a .wasm theme"));
                continue;
            }

            match self.install(path) {
                Ok(()) => installed = true,
                Err(err) => problems.push(format!("Couldn't add {name}: {err}")),
            }
        }

        (installed, problems)
    }

    /// Reinstalls every theme whose source is one of `changed` and was modified since it was
    /// installed. There's a message for each, `Err` if it couldn't be reinstalled.
    pub fn reinstall_changed(&mut self, changed: &[PathBuf]) -> Vec<Result<String, String>> {
        let sources: Vec<PathBuf> = self
            .config_store
            .themes
            .iter()
            .filter(|entry| changed.contains(&entry.source) && is_outdated(entry))
            .map(|entry| entry.source.clone())
            .collect();

        sources
            .into_iter()
            .map(|source| match self.install(&source) {
                Ok(()) => Ok(format!("Reloaded {}", source.display())),
                Err(err) => Err(format!("Couldn't reload {}: {err}", source.display())),
            })
            .collect()
    }

    pub fn move_theme(&mut self, id: &str, target: &str) -> bool {
        let (Some(from), Some(to)) = (self.position(id), self.position(target)) else {
            return false;
        };
        if from == to {
            return false;
        }

        let entry = self.config_store.themes.remove(from);
        self.config_store.themes.insert(to, entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> io::Result<()> {
        let Some(index) = self.position(id) else {
            return Ok(());
        };

        let entry = self.config_store.themes.remove(index);
        self.manifests.remove(id);
        for app in self.config_store.apps.values_mut() {
            app.themes.remove(id);
        }

        match fs::remove_file(entry.binary_path()?) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }
}

pub fn read_manifest(wasm: &[u8]) -> io::Result<OwnedManifest> {
    crate::metadata::read(wasm)
}

fn load_manifest(entry: &ThemeEntry) -> Result<OwnedManifest, String> {
    entry
        .binary_path()
        .and_then(fs::read)
        .and_then(|wasm| {
            let manifest = read_manifest(&wasm)?;
            if manifest.id != entry.id {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "installed theme ID does not match its library entry",
                ));
            }
            Ok(manifest)
        })
        .map_err(|err| err.to_string())
}

/// Whether the theme's source was modified after it was installed.
fn is_outdated(entry: &ThemeEntry) -> bool {
    let modified = |path: &Path| fs::metadata(path).and_then(|meta| meta.modified()).ok();
    let installed = entry.binary_path().ok().and_then(|path| modified(&path));

    match (modified(&entry.source), installed) {
        (Some(source), Some(installed)) => source > installed,
        (source, None) => source.is_some(),
        (None, _) => false,
    }
}

/// An app a frontend injected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryApp {
    /// the ID it's configured under
    pub id: String,
    /// the ID of the installed app that was changed to inject it, like a desktop file ID
    pub installed_app: String,
    pub name: String,
    pub icon: Option<String>,
}

/// What's shown about a theme wherever it's listed.
pub struct ThemeInfo<'a> {
    pub title: String,
    pub subtitle: String,
    /// empty if there isn't one
    pub description: &'a str,
    pub settings: &'a [OwnedSettingsField],
    id: &'a str,
}

impl<'a> ThemeInfo<'a> {
    fn new(entry: &'a ThemeEntry, manifest: Option<&'a Result<OwnedManifest, String>>) -> Self {
        let (title, subtitle) = match manifest {
            Some(Ok(manifest)) => (
                manifest.name.clone(),
                format!("v{} by {}", manifest.version, manifest.author),
            ),
            Some(Err(err)) => (entry.id.clone(), err.clone()),
            None => (entry.id.clone(), String::new()),
        };

        let (description, settings) = match manifest {
            Some(Ok(manifest)) => (manifest.description.as_str(), &manifest.settings[..]),
            _ => ("", &[][..]),
        };

        Self {
            title,
            subtitle,
            description,
            settings,
            id: &entry.id,
        }
    }

    /// Lowercase text for [`matches`] to search.
    pub fn search_text(&self) -> String {
        format!(
            "{} {} {} {}",
            self.title, self.subtitle, self.description, self.id
        )
        .to_lowercase()
    }
}
