//! State tracking for themes and apps.

use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use erebus_backend::ThemeModule;
use erebus_ipc::config::{self, AppTheme, ConfigStore, ThemeEntry};
use erebus_theme::{OwnedManifest, settings::OwnedSettingsField};

#[derive(Default)]
pub struct Library {
    pub config_store: ConfigStore,
    /// keyed by theme id. `Err` holds why the binary couldn't be read
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

    /// Copies the theme at `source` into the themes directory. A theme with the same file
    /// name will replace the existing one but keep its settings.
    pub fn install(&mut self, source: &Path) -> erebus_backend::Result<()> {
        let source = source.canonicalize()?;
        let wasm = fs::read(&source)?;
        let manifest = read_manifest(&wasm)?;

        let id = source
            .file_stem()
            .ok_or_else(|| io::Error::other("theme path has no file name"))?
            .to_string_lossy()
            .into_owned();

        let index = self.position(&id).unwrap_or_else(|| {
            self.config_store.themes.push(ThemeEntry {
                id: id.clone(),
                source: source.clone(),
            });
            self.config_store.themes.len() - 1
        });

        let entry = &mut self.config_store.themes[index];
        entry.source = source;
        config::write_atomic(&entry.binary_path()?, &wasm)?;
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

fn read_manifest(wasm: &[u8]) -> erebus_backend::Result<OwnedManifest> {
    ThemeModule::from_bytes(wasm)?.manifest()
}

fn load_manifest(entry: &ThemeEntry) -> Result<OwnedManifest, String> {
    entry
        .binary_path()
        .and_then(fs::read)
        .map_err(Into::into)
        .and_then(|wasm| read_manifest(&wasm))
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
