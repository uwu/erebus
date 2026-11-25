use std::rc::Rc;

use erebus_frontend::inject::{
    backend,
    installed::{self, Dirs, InstalledApp, State},
};
use zgui::prelude::*;
use zgui::reactive::{LocalStorage, RwSignal, UnsyncCallback};
use zgui::style;
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;

use crate::{components::octicons::SEARCH, state::AppState, ui::app_icon::AppIconProps};

// Dialogs render in the overlay, so attach these styles to the dialog surface.
style! { InjectStyle => r#"
    :scope {
        height: 90%;
        max-height: 640px;
    }

    /* a set height, so the dialog doesn't change size as a search narrows the list */
    :scope .app-sections {
        gap: var(--zui-space-xl);
        width: 100%;
        min-width: 0;
    }

    :scope .app-group {
        border: 1px solid var(--zui-color-border);
        border-radius: var(--zui-radius-md);
        background-color: var(--zui-color-card);
        overflow: hidden;
    }

    /* Overlap the first visible row's border with the card border. Filtering can change which
       row is first. */
    :scope .app-group > .installed-app {
        border-width: 1px 0 0 0;
        border-color: var(--zui-color-border);
        border-radius: 0;
        margin-top: -1px;
    }

    :scope .installed-app.unsupported {
        opacity: 0.55;
    }

    :scope .hidden {
        display: none;
    }
"# }

#[component]
pub fn InjectDialog() -> impl IntoView {
    let app = AppState::get();
    install_stylesheet(InjectStyle::CLASS, InjectStyle::CSS);
    // bumped to scan again once an app that wasn't in the list is injected
    let scans = RwSignal::new_local(0u32);

    view! {
        Dialog(open = app.inject_open) {
            DialogContent(class = InjectStyle::CLASS) {
                DialogHeader {
                    DialogTitle {"Inject erebus"}
                    DialogDescription {
                        "After injecting, close the app and restart it. erebus only works on GPUI applications."
                    }
                }
                {move || {
                    scans.track();
                    AnyView::new(view! { InstalledApps() })
                }}
                {choose_executable(app, scans)}
            }
        }
    }
}

/// Let the user pick an executable when there's no shortcut to scan.
#[cfg(windows)]
fn choose_executable(app: AppState, scans: RwSignal<u32, LocalStorage>) -> AnyView {
    let rescan = move || scans.update(|scans| *scans += 1);
    AnyView::new(view! {
        DialogFooter {
            Button(variant = ButtonVariant::Outline, on:click = move |_| app.pick_executable(rescan)) {
                "Choose an App…"
            }
        }
    })
}

#[cfg(not(windows))]
fn choose_executable(_app: AppState, _scans: RwSignal<u32, LocalStorage>) -> AnyView {
    AnyView::new(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Injected,
    Available,
    Unsupported,
}

impl Section {
    fn of(installed: &InstalledApp) -> Self {
        match installed.state {
            _ if installed.state.is_injected() => Self::Injected,
            // injected, though the app itself is gone
            State::Orphaned => Self::Injected,
            State::Unsupported(_) => Self::Unsupported,
            _ => Self::Available,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Injected => "Injected",
            Self::Available => "Available",
            Self::Unsupported => "Not Supported",
        }
    }
}

/// Scan again each time the dialog opens; installed apps may have changed.
#[component]
fn InstalledApps() -> impl IntoView {
    let app = AppState::get();
    let dirs = match Dirs::from_env() {
        Ok(dirs) => dirs,
        Err(err) => {
            app.error(format!("Couldn't look for apps: {err}"));
            return AnyView::new(view! {
                DialogDescription {"Couldn't look for apps."}
            });
        }
    };

    let found = installed::scan(&dirs);

    let bundled = backend::bundled();
    if let Err(missing) = &bundled {
        eprintln!("{missing}");
    }
    let can_inject = bundled.is_ok();

    let query = RwSignal::new_local(String::new());
    let dirs = Rc::new(dirs);
    let rows: Vec<_> = found
        .into_iter()
        .map(|installed| {
            let haystack = format!(
                "{} {} {}",
                installed.name,
                installed.id,
                installed.program.as_deref().unwrap_or_default()
            )
            .to_lowercase();
            (installed, haystack)
        })
        .collect();
    let haystacks: Vec<String> = rows.iter().map(|(_, haystack)| haystack.clone()).collect();
    let any_match = move || {
        let query = query.get().trim().to_lowercase();
        haystacks.iter().any(|haystack| haystack.contains(&query))
    };

    let matches =
        move |haystack: &str| haystack.contains(query.get().trim().to_lowercase().as_str());
    // Keep rows in their original sections while the dialog is open. Moving a row as soon as
    // it's toggled would pull it out from under the pointer.
    let sections: Vec<_> = [Section::Injected, Section::Available, Section::Unsupported]
        .into_iter()
        .filter_map(|section| {
            let members: Vec<_> = rows
                .iter()
                .filter(|(installed, _)| Section::of(installed) == section)
                .cloned()
                .collect();
            if members.is_empty() {
                return None;
            }
            let haystacks: Vec<String> = members
                .iter()
                .map(|(_, haystack)| haystack.clone())
                .collect();
            let shown = move || haystacks.iter().any(|haystack| matches(haystack));
            let rows: Vec<_> = members
                .into_iter()
                .map(|(installed, haystack)| {
                    view! {
                        InstalledAppRow(
                            installed = installed,
                            dirs = dirs.clone(),
                            can_inject = can_inject,
                            visible = Signal::derive_local(move || matches(&haystack))
                        )
                    }
                    .into_view()
                })
                .collect();
            Some(
                view! {
                    column(class:hidden = move || !shown()) {
                        SettingsGroup {
                            SettingsGroupLabel {{section.label()}}
                            ItemGroup(class = "app-group") {{rows}}
                        }
                    }
                }
                .into_view(),
            )
        })
        .collect();

    AnyView::new(view! {
        if move || !can_inject {
            Alert(variant = AlertVariant::Destructive) {
                AlertTitle {{format!("erebus's {} is missing, so apps can't be injected", backend::NAME)}}
            }
        } else {}
        InputGroup {
            InputGroupAddon {Icon(icon = SEARCH, label = "")}
            InputGroupInput(
                value = query,
                on_change = UnsyncCallback::new(move |text: String| query.set(text)),
                placeholder = "Search apps",
                a11y:label = "Search apps"
            )
        }
        scroll(class = "app-list") {
            column(class = "app-sections") {
                {sections}
                if move || !any_match() {
                    Empty {
                        EmptyHeader {
                            EmptyTitle {"No Results Found"}
                            EmptyDescription {"Try a different search"}
                        }
                    }
                } else {}
            }
        }
    })
}

#[component]
fn InstalledAppRow(
    installed: InstalledApp,
    dirs: Rc<Dirs>,
    can_inject: bool,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    let app = AppState::get();
    let icon = installed.icon.clone();
    let name = installed.name.clone();

    let detail = match &installed.state {
        State::Unsupported(reason) => reason.to_string(),
        State::Orphaned => "Uninstalled, but still injected".to_owned(),
        _ => installed.detail(),
    };

    let unsupported = matches!(installed.state, State::Unsupported(_));
    let orphaned = installed.state == State::Orphaned;
    let installed = RwSignal::new_local(installed);
    let removed = RwSignal::new_local(false);

    let set_injected = {
        let dirs = dirs.clone();
        move |inject: bool| set_injected(app, &dirs, installed, inject)
    };
    let toggle = Rc::new(move |inject: bool| {
        // uninjecting wipes an app's theme preferences, so ask first if it has any
        let app_id = installed.with_untracked(InstalledApp::app_id);
        if !inject && app.has_preferences(&app_id) {
            let set_injected = set_injected.clone();
            app.confirm_remove_erebus(&app_id, move || set_injected(false));
        } else {
            set_injected(inject);
        }
    });

    let remove = move |_: &mut EventCx<'_, zgui::view::events::Click>| {
        let current = installed.get_untracked();
        match installed::uninject(&dirs, &current) {
            Ok(()) => {
                removed.set(true);
                app.uninjected(&current.app_id());
            }
            Err(err) => app.error(format!("Couldn't remove it: {err}")),
        }
    };

    let injected =
        Signal::derive_local(move || installed.with(|installed| installed.state.is_injected()));
    let checked = Binding::controlled(injected, move |want: bool| toggle(want));

    let action = if unsupported {
        AnyView::new(())
    } else if orphaned {
        AnyView::new(view! {
            Button(variant = ButtonVariant::Outline, size = ButtonSize::Sm, on:click = remove) {
                "Remove"
            }
        })
    } else {
        AnyView::new(view! {
            Switch(
                checked = checked,
                // something that's injected can always be uninjected
                disabled = Signal::derive_local(move || !can_inject && !injected.get()),
                a11y:label = name.clone()
            )
        })
    };

    view! {
        Item(
            size = ItemSize::Sm,
            class = "installed-app",
            class:unsupported = unsupported,
            class:hidden = move || !visible.get() || removed.get()
        ) {
            ItemMedia {AppIcon(icon = icon, size = 32.0)}
            ItemContent {
                ItemTitle {{installed.with_untracked(|installed| installed.name.clone())}}
                ItemDescription {{detail}}
            }
            ItemActions {{action}}
        }
    }
}

fn set_injected(
    app: AppState,
    dirs: &Dirs,
    installed: RwSignal<InstalledApp, LocalStorage>,
    inject: bool,
) {
    if let Some(updated) = app.set_injected(dirs, &installed.get_untracked(), inject) {
        installed.set(updated);
    }
}
