use std::rc::Rc;

use zgui::prelude::*;

use super::{AppState, Confirm, Selection};

/// An app as it's listed in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppEntry {
    pub name: String,
    pub app_id: String,
    pub icon: Option<String>,
}

impl AppState {
    /// The sidebar list is sorted by app name.
    pub fn apps(self) -> Vec<AppEntry> {
        let mut apps: Vec<AppEntry> = self.library.with(|library| {
            self.runtime.with(|runtime| {
                library
                    .config_store
                    .apps
                    .keys()
                    .chain(runtime.running())
                    .map(|app_id| AppEntry {
                        name: library.app_name(app_id).to_owned(),
                        app_id: app_id.clone(),
                        icon: library.app_icon(app_id).map(str::to_owned),
                    })
                    .collect()
            })
        });
        apps.sort();
        apps.dedup();
        apps
    }

    /// Falls back to the library if the selected app is gone from the sidebar.
    pub fn fix_selection(self) {
        let Selection::App(app_id) = self.selection.get_untracked() else {
            return;
        };
        let listed = self
            .library
            .with_untracked(|library| library.config_store.apps.contains_key(&app_id))
            || self
                .runtime
                .with_untracked(|runtime| !runtime.instances(&app_id).is_empty());
        if !listed {
            self.selection.set(Selection::Library);
        }
    }

    /// Removing erebus deletes these preferences, so check before doing it without
    /// confirmation.
    pub fn has_preferences(self, app_id: &str) -> bool {
        self.library.with_untracked(|library| {
            library
                .config_store
                .apps
                .get(app_id)
                .is_some_and(|app| !app.themes.is_empty())
        })
    }

    /// An app can remain in the sidebar after removal if it's still running. Don't offer to
    /// remove it again.
    pub fn is_removable(self, app_id: &str) -> bool {
        self.library
            .with(|library| library.config_store.apps.contains_key(app_id))
            || self.instances(app_id) == 0
    }

    /// Asks before removing erebus from an app, since that wipes its theme preferences. If the
    /// app is running, this is also where the user is told to relaunch it.
    pub fn confirm_remove_erebus(self, app_id: &str, on_confirm: impl Fn() + 'static) {
        let name = self
            .library
            .with_untracked(|library| library.app_name(app_id).to_owned());

        let mut body =
            vec!["Any theme settings for this application will also be removed.".to_owned()];
        if self
            .runtime
            .with_untracked(|runtime| !runtime.instances(app_id).is_empty())
        {
            body.push(format!(
                "{name} is currently running with erebus loaded. After removing, relaunch the app to finish removing erebus."
            ));
        }

        self.ask(Confirm {
            title: format!("Remove erebus from {name}?"),
            body,
            action: "Remove".to_owned(),
            on_confirm: Rc::new(on_confirm),
        });
    }

    /// Ask with [`confirm_remove_erebus`](Self::confirm_remove_erebus) before clearing the
    /// app's config and removing its injection.
    ///
    /// A running app loses its themes immediately. Erebus stays loaded until the app is
    /// relaunched, so we keep it in the sidebar for now.
    pub fn forget_app(self, app_id: &str) {
        if !self.uninject(app_id) {
            return;
        }

        self.library.update(|library| {
            library.config_store.apps.remove(app_id);
        });

        if self
            .runtime
            .with_untracked(|runtime| runtime.instances(app_id).is_empty())
        {
            self.runtime.update(|runtime| runtime.forget(app_id));
            self.save();
        } else {
            // tells its backends there's nothing to apply anymore
            self.commit(Some(app_id));
        }
        self.fix_selection();
    }
}
