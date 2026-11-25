//! Injecting erebus into installed apps.

use erebus_frontend::inject::{
    backend,
    installed::{self, Dirs, InstalledApp},
};
use zgui::prelude::*;

use super::{AppState, Selection};

impl AppState {
    /// The frontend or an injected app may have updated since we last ran. Refresh the backend
    /// copies first, then rebuild the sidebar from the current injection state.
    pub fn sync_injected(self) {
        match backend::update() {
            Ok(true) => eprintln!("updated the installed {}", backend::NAME),
            Ok(false) => {}
            Err(err) => self.error(format!("Couldn't update erebus in your apps: {err}")),
        }

        let Ok(dirs) = Dirs::from_env() else {
            return;
        };
        let (updated, problems) = installed::update_all(&dirs, &installed::scan(&dirs));
        for name in updated {
            eprintln!("updated erebus in {name}");
        }
        for problem in problems {
            self.error(problem);
        }

        let injected: Vec<_> = installed::scan(&dirs)
            .iter()
            .filter(|installed| installed.state.is_injected())
            .map(InstalledApp::library_app)
            .collect();
        if self.edit(|library| library.sync_apps(&injected)) {
            self.save();
        }
    }

    /// Try to remove the app's injection before forgetting it. Return false if it's still
    /// injected so the caller keeps its config.
    pub fn uninject(self, app_id: &str) -> bool {
        let Some(id) = self
            .library
            .with_untracked(|library| library.installed_app(app_id).map(str::to_owned))
        else {
            return true;
        };

        let dirs = match Dirs::from_env() {
            Ok(dirs) => dirs,
            Err(err) => {
                self.error(format!("Couldn't look for apps: {err}"));
                return false;
            }
        };

        let found = installed::scan(&dirs)
            .into_iter()
            .find(|installed| installed.id == id && installed.state.is_injected());
        // already uninjected some other way
        let Some(found) = found else {
            return true;
        };

        match installed::uninject(&dirs, &found) {
            Ok(()) => true,
            Err(err) => {
                self.error(format!("Couldn't uninject {}: {err}", found.name));
                false
            }
        }
    }

    /// Let the user pick an executable, then rescan after injecting it.
    #[cfg(windows)]
    pub fn pick_executable(self, rescan: impl Fn() + 'static) {
        spawn_local(async move {
            let picked = background(async {
                rfd::FileDialog::new()
                    .set_title("Choose an App")
                    .add_filter("Programs", &["exe"])
                    .pick_file()
            })
            .await;
            let Some(path) = picked else {
                return;
            };

            let found = Dirs::from_env()
                .map_err(installed::Error::from)
                .and_then(|dirs| Ok((installed::executable(&dirs, &path)?, dirs)));
            match found {
                Ok((found, _)) if found.state.is_injected() => {
                    self.toast(format!("{} is already injected.", found.name));
                }
                Ok((found, dirs)) => {
                    self.set_injected(&dirs, &found, true);
                    rescan();
                }
                Err(err) => self.error(format!("Couldn't inject {}: {err}", path.display())),
            }
        });
    }

    /// Returns the updated app when its injection state changes.
    pub fn set_injected(
        self,
        dirs: &Dirs,
        current: &InstalledApp,
        inject: bool,
    ) -> Option<InstalledApp> {
        let result = if inject {
            install_backend()
                .and_then(|()| installed::inject(dirs, current).map_err(|err| err.to_string()))
        } else {
            installed::uninject(dirs, current).map_err(|err| err.to_string())
        };

        match result {
            Ok(()) => {
                // injecting can move it
                let updated = installed::scan(dirs)
                    .into_iter()
                    .find(|installed| installed.id == current.id)
                    .unwrap_or_else(|| current.clone());
                if inject {
                    if self.edit(|library| library.set_app(&updated.library_app())) {
                        self.save();
                    }
                    self.toast(format!(
                        "{} will be injected the next time it starts.",
                        current.name
                    ));
                } else {
                    self.uninjected(&current.app_id());
                }
                Some(updated)
            }
            Err(err) => {
                let verb = if inject { "inject" } else { "uninject" };
                self.error(format!("Couldn't {verb} {}: {err}", current.name));
                None
            }
        }
    }

    /// Clear the uninjected app's themes now. If it's still running, keep it in the sidebar
    /// until it quits.
    pub fn uninjected(self, app_id: &str) {
        if !self.edit(|library| library.config_store.apps.remove(app_id).is_some()) {
            return;
        }

        self.commit(Some(app_id));
        if self.selection.get_untracked() == Selection::App(app_id.to_owned()) {
            self.fix_selection();
        }
    }
}

fn install_backend() -> Result<(), String> {
    let bundled = backend::bundled().map_err(|err| err.to_string())?;
    backend::install(&bundled)
        .map(|_| ())
        .map_err(|err| format!("couldn't install the {}: {err}", backend::NAME))
}
