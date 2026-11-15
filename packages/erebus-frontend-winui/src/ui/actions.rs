//! Changes to the config store made from the UI.

use std::{path::PathBuf, process::Command};

use erebus_frontend::{library::Library, watcher};
use erebus_theme::settings::SettingValue;
use windows_reactor::*;

use super::{Erebus, Message, Selection};
use crate::{
    inbox::ThreadEvent,
    logs::{LogWindow, LogWindowInput},
};

// menu items. the menus hand back the label of whatever was clicked, so these double as ids
pub const OPEN_CONFIG_FOLDER: &str = "Open Config Folder";
pub const RELOAD_LIBRARY: &str = "Reload Library";
pub const MOVE_UP: &str = "Move Up";
pub const MOVE_DOWN: &str = "Move Down";
pub const REMOVE: &str = "Remove…";
pub const VIEW_LOGS: &str = "View Logs";
pub const FORGET_APP: &str = "Forget App";

impl Erebus {
    //
    // MARK: saving
    //

    pub(super) fn save(&mut self, context: &ComponentContext<Self>) -> bool {
        match self.library.config_store.save() {
            Ok(()) => true,
            Err(err) => {
                self.notify_error(format!("Couldn't save: {err}"), context);
                false
            }
        }
    }

    /// Saves the store, then tells the backends of `process` (or every backend, if it's
    /// `None`) to go read it.
    fn commit(&mut self, process: Option<&str>, context: &ComponentContext<Self>) {
        if self.save(context)
            && let Some(ipc) = &self.ipc
        {
            ipc.sync(process);
        }
    }

    /// Call this whenever something about the library itself changes.
    fn library_changed(&mut self, context: &ComponentContext<Self>) {
        self.commit(None, context);
        self.watch(context);
    }

    //
    // MARK: app themes
    //

    pub(super) fn set_enabled(
        &mut self,
        process: &str,
        id: &str,
        enabled: bool,
        context: &ComponentContext<Self>,
    ) {
        self.library.app_theme_mut(process, id).enabled = enabled;
        self.commit(Some(process), context);
    }

    pub(super) fn set_setting(
        &mut self,
        process: &str,
        id: &str,
        key: String,
        value: SettingValue,
        context: &ComponentContext<Self>,
    ) {
        self.library
            .app_theme_mut(process, id)
            .settings
            .insert(key, value);
        self.commit(Some(process), context);
    }

    pub(super) fn app_menu(&mut self, process: &str, item: &str, context: &ComponentContext<Self>) {
        match item {
            VIEW_LOGS => {
                let input = LogWindowInput {
                    process: process.to_owned(),
                    logs: self.runtime.logs(process),
                };
                if !context.open_window(View::component::<LogWindow>(input)) {
                    self.notify_error("Couldn't open the logs window".to_owned(), context);
                }
            }
            // drops everything we have stored for an app that isn't running
            FORGET_APP => {
                self.library.config_store.apps.remove(process);
                self.runtime.forget(process);
                self.save(context);
                self.selection = Selection::Library;
            }
            _ => {}
        }
    }

    //
    // MARK: library
    //

    pub(super) fn pick_themes(&self, context: &ComponentContext<Self>) {
        _ = context.spawn_background(|_| {
            // the dialog gets a thread of its own, so it can set up COM however it likes
            let picked = std::thread::spawn(|| {
                rfd::FileDialog::new()
                    .set_title("Add Theme")
                    .add_filter("WebAssembly themes", &["wasm"])
                    .pick_files()
            })
            .join();

            Message::Picked(picked.ok().flatten().unwrap_or_default())
        });
    }

    /// Adds theme binaries to the library, skipping anything that isn't a `.wasm` file.
    pub(super) fn install_themes(&mut self, paths: &[PathBuf], context: &ComponentContext<Self>) {
        let (installed, problems) = self.library.install_files(paths);
        for problem in problems {
            self.notify_error(problem, context);
        }

        if installed {
            self.library_changed(context);
        }
    }

    pub(super) fn install_dropped(&mut self, data: DroppedData, context: &ComponentContext<Self>) {
        match data {
            DroppedData::StorageItems(items) => {
                let paths: Vec<PathBuf> = items.into_iter().map(|item| item.path.into()).collect();
                self.install_themes(&paths, context);
            }
            _ => self.notify_error("Only theme files can be dropped here".to_owned(), context),
        }
    }

    pub(super) fn library_menu(&mut self, item: &str, context: &ComponentContext<Self>) {
        match item {
            OPEN_CONFIG_FOLDER => self.open_config_folder(context),
            RELOAD_LIBRARY => self.reload_library(context),
            _ => {}
        }
    }

    pub(super) fn theme_menu(&mut self, id: &str, item: &str, context: &ComponentContext<Self>) {
        let themes = &self.library.config_store.themes;
        let Some(index) = themes.iter().position(|entry| entry.id == id) else {
            return;
        };

        let neighbor = match item {
            MOVE_UP => index.checked_sub(1),
            MOVE_DOWN => Some(index + 1),
            REMOVE => {
                let name = self.library.theme_info(&themes[index]).title;
                self.removing = Some((id.to_owned(), name));
                return;
            }
            _ => None,
        };

        let Some(target) = neighbor.and_then(|index| themes.get(index)) else {
            return;
        };

        let target = target.id.clone();
        if self.library.move_theme(id, &target) {
            self.commit(None, context);
        }
    }

    pub(super) fn remove_theme(&mut self, id: &str, context: &ComponentContext<Self>) {
        if let Err(err) = self.library.remove(id) {
            self.notify_error(format!("Couldn't remove theme binary: {err}"), context);
        }

        self.library_changed(context);
    }

    //
    // MARK: files on disk
    //

    fn open_config_folder(&mut self, context: &ComponentContext<Self>) {
        let opened = erebus_ipc::config::config_dir().and_then(|dir| {
            // the folder doesn't exist until something's been saved
            std::fs::create_dir_all(&dir)?;
            Command::new("explorer").arg(&dir).spawn()
        });

        if let Err(err) = opened {
            self.notify_error(format!("Couldn't open the config folder: {err}"), context);
        }
    }

    /// Reads the library back from disk (say, after someone edited the config by hand) and
    /// has every backend do the same.
    fn reload_library(&mut self, context: &ComponentContext<Self>) {
        match Library::load() {
            Ok(library) => self.library = library,
            Err(err) => {
                self.notify_error(format!("Couldn't reload the theme library: {err}"), context);
                return;
            }
        }

        if let Some(ipc) = &self.ipc {
            ipc.sync(None);
        }

        self.watch(context);
        self.notify(
            "Reloaded the theme library".to_owned(),
            InfoBarSeverity::Success,
            context,
        );
    }

    /// Reinstalls any theme whose source is in `paths` and has changed.
    pub(super) fn reload_sources(&mut self, paths: &[PathBuf], context: &ComponentContext<Self>) {
        let reloaded = self.library.reinstall_changed(paths);
        if reloaded.is_empty() {
            return;
        }

        for message in reloaded {
            match message {
                Ok(message) => self.notify(message, InfoBarSeverity::Informational, context),
                Err(message) => self.notify_error(message, context),
            }
        }

        self.library_changed(context);
    }

    /// Swaps the watcher out for one that covers the library's current theme sources.
    pub(super) fn watch(&mut self, context: &ComponentContext<Self>) {
        let sender = self.inbox.sender();
        let watcher = watcher::watch_sources(&self.library.config_store.themes, move |paths| {
            let _ = sender.send(ThreadEvent::SourcesChanged(paths));
        });

        match watcher {
            Ok(watcher) => self.watcher = Some(watcher),
            Err(err) => self.notify(
                format!("Couldn't watch themes for changes: {err}"),
                InfoBarSeverity::Warning,
                context,
            ),
        }
    }
}
