use std::{path::PathBuf, rc::Rc};

use erebus_frontend::{library::Library, watcher};
use erebus_theme::settings::SettingValue;
use zgui::prelude::*;

use super::{AppState, Confirm};

impl AppState {
    pub fn save(self) -> bool {
        match self
            .library
            .with_untracked(|library| library.config_store.save())
        {
            Ok(()) => true,
            Err(err) => {
                self.error(format!("Couldn't save: {err}"));
                false
            }
        }
    }

    /// Save before notifying backends so they read the new config. `None` notifies every app.
    pub fn commit(self, app_id: Option<&str>) {
        if self.save() {
            self.ipc.with_value(|ipc| {
                if let Some(ipc) = ipc {
                    ipc.sync(app_id);
                }
            });
        }
    }

    /// Call after anything in the library itself changes.
    fn library_changed(self) {
        self.commit(None);
        self.watch();
    }

    pub fn set_enabled(self, app_id: &str, id: &str, enabled: bool) {
        self.library.update_untracked(|library| {
            library.app_theme_mut(app_id, id).enabled = enabled;
        });
        self.commit(Some(app_id));
    }

    pub fn set_setting(self, app_id: &str, id: &str, key: &str, value: SettingValue) {
        self.library.update_untracked(|library| {
            library
                .app_theme_mut(app_id, id)
                .settings
                .insert(key.to_owned(), value);
        });
        self.commit(Some(app_id));
    }

    pub fn pick_theme(self) {
        spawn_local(async move {
            let picked = background(async {
                rfd::FileDialog::new()
                    .set_title("Add Theme")
                    .add_filter("WebAssembly files", &["wasm"])
                    .pick_files()
            })
            .await;

            if let Some(paths) = picked {
                self.install_themes(&paths);
            }
        });
    }

    /// Adds theme binaries to the library, skipping anything that isn't a `.wasm` file.
    pub fn install_themes(self, paths: &[PathBuf]) {
        let mut problems = Vec::new();
        let installed = self.edit(|library| {
            let (installed, found) = library.install_files(paths);
            problems = found;
            installed
        });
        if installed {
            self.library_changed();
        }

        for problem in problems {
            self.error(problem);
        }
    }

    /// Moves a theme to the position of `target` in the library.
    pub fn move_theme(self, id: &str, target: &str) {
        if self.edit(|library| library.move_theme(id, target)) {
            self.commit(None);
        }
    }

    pub fn confirm_remove_theme(self, id: &str, name: &str) {
        let id = id.to_owned();
        self.ask(Confirm {
            title: "Remove Theme?".to_owned(),
            body: vec![format!(
                "\"{name}\" will be removed from the library, along with its settings in every app."
            )],
            action: "Remove".to_owned(),
            on_confirm: Rc::new(move || self.remove_theme(&id)),
        });
    }

    fn remove_theme(self, id: &str) {
        let mut removed = Ok(());
        self.library.update(|library| removed = library.remove(id));
        if let Err(err) = removed {
            self.error(format!("Couldn't remove theme binary: {err}"));
        }

        self.library_changed();
    }

    pub fn open_config_folder(self) {
        let dir = match erebus_ipc::config::config_dir() {
            Ok(dir) => dir,
            Err(err) => {
                self.error(format!("Couldn't find the config folder: {err}"));
                return;
            }
        };

        // it only exists once something has been saved
        if let Err(err) = std::fs::create_dir_all(&dir) {
            self.error(format!("Couldn't create {}: {err}", dir.display()));
            return;
        }

        self.open_external(&dir.to_string_lossy());
    }

    /// Reload from disk after an external edit. Backends need to reload too.
    pub fn reload_library(self) {
        match Library::load() {
            Ok(library) => self.library.set(library),
            Err(err) => {
                self.error(format!("Couldn't reload the theme library: {err}"));
                return;
            }
        }

        self.ipc.with_value(|ipc| {
            if let Some(ipc) = ipc {
                ipc.sync(None);
            }
        });

        self.watch();
        self.fix_selection();
        self.toast("Reloaded the theme library");
    }

    /// Reinstalls every theme whose source is one of `paths` and has changed.
    pub(super) fn reload_sources(self, paths: &[PathBuf]) {
        let mut reloaded = Vec::new();
        self.edit(|library| {
            reloaded = library.reinstall_changed(paths);
            !reloaded.is_empty()
        });
        if reloaded.is_empty() {
            return;
        }

        for message in reloaded {
            match message {
                Ok(message) => self.toast(message),
                Err(message) => self.error(message),
            }
        }

        self.library_changed();
    }

    /// Replaces the watcher with one for the library's current theme sources.
    pub(super) fn watch(self) {
        let sender = self.on_sources_changed.get_value();
        let watcher = self.library.with_untracked(|library| {
            watcher::watch_sources(&library.config_store.themes, move |paths| {
                let _ = sender.send_blocking(paths);
            })
        });

        match watcher {
            Ok(watcher) => self.watcher.set_value(Some(watcher)),
            Err(err) => self.error(format!("Couldn't watch themes for changes: {err}")),
        }
    }
}
