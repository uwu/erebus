//! Application state

mod apps;
mod instances;
mod inject;
mod library;
mod nav;

pub use apps::AppEntry;
pub use instances::{Line, LogBuffer};
pub use nav::{Nav, Selection};

use std::{collections::HashSet, path::PathBuf, process::Command, rc::Rc};

use erebus_frontend::{apps::Runtime, ipc::IpcFrontend, library::Library};
use zgui::prelude::*;
use zgui::reactive::{LocalStorage, RwSignal, StoredValue, Trigger, spawn_stream};
use zgui_ui::prelude::*;

#[derive(Clone)]
pub struct Confirm {
    pub title: String,
    pub body: Vec<String>,
    pub action: String,
    pub on_confirm: Rc<dyn Fn()>,
}

#[derive(Clone, Copy)]
pub struct AppState {
    /// Only notify for structural changes. Rebuilding a page on every setting change would
    /// recreate the control the user is still interacting with.
    pub library: RwSignal<Library, LocalStorage>,
    pub runtime: RwSignal<Runtime<LogBuffer>, LocalStorage>,
    pub logs_changed: Trigger,

    pub selection: RwSignal<Selection, LocalStorage>,
    /// kept across pages
    pub library_search: RwSignal<String, LocalStorage>,
    pub confirm: RwSignal<Option<Confirm>, LocalStorage>,
    pub inject_open: RwSignal<bool, LocalStorage>,
    pub about_open: RwSignal<bool, LocalStorage>,
    /// Show one pane at a time when both won't fit.
    pub narrow: Signal<bool, LocalStorage>,
    /// Only used in narrow windows; wide windows show both panes.
    pub showing_page: RwSignal<bool, LocalStorage>,
    /// Keep the navigation direction until the slide animation finishes.
    pub nav: RwSignal<Option<Nav>, LocalStorage>,
    /// Remember expanded settings across page changes, keyed by `app_id/theme`.
    expanded: StoredValue<HashSet<String>, LocalStorage>,

    ipc: StoredValue<Option<IpcFrontend>, LocalStorage>,
    watcher: StoredValue<Option<erebus_frontend::watcher::Watcher>, LocalStorage>,
    on_sources_changed: StoredValue<async_channel::Sender<Vec<PathBuf>>, LocalStorage>,
    windows: StoredValue<Windows, LocalStorage>,
    toasts: Option<ToastQueue>,
}

impl AppState {
    /// Call inside the toaster so startup errors can be shown. The resulting state is available
    /// through [`AppState::get`].
    pub fn provide() -> Self {
        let (events_tx, events_rx) = async_channel::unbounded();
        let (sources_tx, sources_rx) = async_channel::unbounded();

        let (library, library_err) = match Library::load() {
            Ok(library) => (library, None),
            Err(err) => (Library::default(), Some(err)),
        };

        let on_event = move |event| {
            let _ = events_tx.send_blocking(event);
        };
        let (ipc, ipc_err) = match IpcFrontend::start(on_event) {
            Ok(ipc) => {
                if let Ok(name) = erebus_ipc::transport::socket_name() {
                    eprintln!("listening on {name:?}");
                }
                (Some(ipc), None)
            }
            Err(err) => (None, Some(err)),
        };

        let app = Self {
            library: RwSignal::new_local(library),
            runtime: RwSignal::new_local(Runtime::default()),
            logs_changed: Trigger::new(),
            selection: RwSignal::new_local(Selection::Library),
            library_search: RwSignal::new_local(String::new()),
            confirm: RwSignal::new_local(None),
            inject_open: RwSignal::new_local(false),
            about_open: RwSignal::new_local(false),
            narrow: nav::narrow_window(),
            showing_page: RwSignal::new_local(false),
            nav: RwSignal::new_local(None),
            expanded: StoredValue::new_local(HashSet::new()),
            ipc: StoredValue::new_local(ipc),
            watcher: StoredValue::new_local(None),
            on_sources_changed: StoredValue::new_local(sources_tx),
            windows: StoredValue::new_local(use_windows()),
            toasts: use_toaster(),
        };
        provide_local_context(app);

        if let Some(err) = library_err {
            app.error(format!("Couldn't load the theme library: {err}"));
        }
        if let Some(err) = ipc_err {
            app.error(format!("Can't talk to apps: {err}"));
        }

        spawn_stream(events_rx, move |event| app.handle_event(event));
        spawn_stream(sources_rx, move |paths: Vec<PathBuf>| {
            app.reload_sources(&paths)
        });

        app.sync_injected();
        app.watch();
        app.follow_navigation();
        app
    }

    pub fn get() -> Self {
        use_local_context::<Self>().expect("the app is provided at the root")
    }

    pub fn toast(self, message: impl Into<String>) {
        if let Some(toasts) = self.toasts {
            toasts.push(Toast::new(message));
        }
    }

    pub fn error(self, message: impl Into<String>) {
        if let Some(toasts) = self.toasts {
            toasts.push(Toast::new(message).kind(ToastKind::Error));
        }
    }

    pub fn ask(self, confirm: Confirm) {
        self.confirm.set(Some(confirm));
    }

    pub fn windows(self) -> Windows {
        self.windows.get_value()
    }

    pub fn is_expanded(self, key: &str) -> bool {
        self.expanded.with_value(|expanded| expanded.contains(key))
    }

    pub fn set_expanded(self, key: &str, expanded: bool) {
        self.expanded.update_value(|set| {
            if expanded {
                set.insert(key.to_owned());
            } else {
                set.remove(key);
            }
        });
    }

    /// Changes the library and rebuilds whatever shows it if `edit` returns true.
    pub fn edit(self, edit: impl FnOnce(&mut Library) -> bool) -> bool {
        let mut changed = false;
        self.library.maybe_update(|library| {
            changed = edit(library);
            changed
        });
        changed
    }

    pub fn open_external(self, target: &str) {
        let opener = if cfg!(windows) {
            "explorer"
        } else {
            "xdg-open"
        };
        if let Err(err) = Command::new(opener).arg(target).spawn() {
            self.error(format!("Couldn't open {target}: {err}"));
        }
    }
}
