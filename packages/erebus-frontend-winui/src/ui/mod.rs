mod actions;
mod events;
mod pages;
mod sidebar;
pub(crate) mod widgets;

use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Duration};

use erebus_frontend::{apps::Runtime, ipc::IpcFrontend, library::Library, watcher::Watcher};
use erebus_theme::settings::SettingValue;
use windows_reactor::*;

use crate::{
    inbox::{Inbox, ThreadEvent},
    logs::Logs,
};

/// how long a notice sticks around before it goes away on its own
const NOTICE_TIMEOUT: Duration = Duration::from_secs(6);

/// height of the title bar we draw over the window's content
const TITLE_BAR_HEIGHT: f64 = 32.0;
/// below this window width, the pane shrinks down to just icons
const COMPACT_WIDTH: f64 = 760.0;
const PANE_WIDTH: f64 = 280.0;
const COMPACT_PANE_WIDTH: f64 = 48.0;
const MAX_PAGE_WIDTH: f64 = 1000.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Library,
    App(String),
}

impl Selection {
    fn tag(&self) -> String {
        match self {
            Self::Library => "library".to_owned(),
            Self::App(process) => format!("app:{process}"),
        }
    }

    fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "library" => Some(Self::Library),
            _ => tag
                .strip_prefix("app:")
                .map(|process| Self::App(process.to_owned())),
        }
    }
}

/// Toast message
struct Notice {
    id: u64,
    text: String,
    severity: InfoBarSeverity,
}

pub struct Erebus {
    library: Library,
    runtime: Runtime<Logs>,
    ipc: Option<IpcFrontend>,
    inbox: Inbox,
    watcher: Option<Watcher>,

    selection: Selection,
    /// whether the window was wider than [`COMPACT_WIDTH`] last we checked
    wide: bool,
    pane_open: bool,
    window_width: f64,
    /// what's typed into the library's search box
    search: String,
    /// `<app>/<theme id>` for every theme whose settings are expanded
    expanded: HashSet<String>,
    /// `(id, name)` of a theme we're asking about removing
    removing: Option<(String, String)>,
    about_open: bool,
    /// whether files are being dragged over the library
    drop_hover: bool,
    notices: Vec<Notice>,
    next_notice: u64,
}

#[derive(Clone)]
pub enum Message {
    Inbox(Arc<Vec<ThreadEvent>>),
    Dismiss(u64),
    Navigate(Option<String>),
    Resized(WindowSize),
    TogglePane,
    PaneOpen(bool),

    Search(String),
    PickThemes,
    Picked(Vec<PathBuf>),
    DropHover(bool),
    Dropped(DroppedData),
    LibraryMenu(String),
    ThemeMenu {
        id: String,
        item: String,
    },
    RemoveClosed(ContentDialogResult),

    AppMenu {
        process: String,
        item: String,
    },
    SetEnabled {
        process: String,
        id: String,
        enabled: bool,
    },
    SetSetting {
        process: String,
        id: String,
        key: String,
        value: SettingValue,
    },
    Expanded {
        key: String,
        expanded: bool,
    },

    About(bool),
}

impl Component for Erebus {
    type Input = ();
    type Message = Message;

    fn create(_input: &(), context: &ComponentContext<Self>) -> Self {
        let inbox = Inbox::new();

        let sender = inbox.sender();
        let (ipc, ipc_err) = match IpcFrontend::start(move |event| {
            let _ = sender.send(ThreadEvent::Ipc(event));
        }) {
            Ok(ipc) => (Some(ipc), None),
            Err(err) => (None, Some(err)),
        };

        let (library, library_err) = match Library::load() {
            Ok(library) => (library, None),
            Err(err) => (Library::default(), Some(err)),
        };

        let mut erebus = Self {
            library,
            runtime: Runtime::default(),
            ipc,
            inbox,
            watcher: None,
            selection: Selection::Library,
            wide: true,
            pane_open: true,
            window_width: 1040.0,
            search: String::new(),
            expanded: HashSet::new(),
            removing: None,
            about_open: false,
            drop_hover: false,
            notices: Vec::new(),
            next_notice: 0,
        };

        if let Some(err) = library_err {
            erebus.notify_error(format!("Couldn't load the theme library: {err}"), context);
        }
        if let Some(err) = ipc_err {
            erebus.notify_error(format!("Can't talk to apps: {err}"), context);
        }

        erebus.watch(context);
        erebus.wait_for_inbox(context);
        erebus
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        match message {
            Message::Inbox(events) => {
                for event in events.iter() {
                    self.handle_thread_event(event, context);
                }
                self.wait_for_inbox(context);
            }
            Message::Dismiss(id) => self.notices.retain(|notice| notice.id != id),
            Message::Navigate(tag) => {
                if let Some(selection) = tag.as_deref().and_then(Selection::from_tag) {
                    self.selection = selection;
                }
            }
            Message::Resized(size) => {
                self.window_width = size.width;
                let wide = size.width >= COMPACT_WIDTH;
                if wide != self.wide {
                    self.wide = wide;
                    self.pane_open = wide;
                }
            }
            Message::TogglePane => self.pane_open = !self.pane_open,
            Message::PaneOpen(open) => self.pane_open = open,

            Message::Search(search) => self.search = search,
            Message::PickThemes => self.pick_themes(context),
            Message::Picked(paths) => self.install_themes(&paths, context),
            Message::DropHover(hover) => self.drop_hover = hover,
            Message::Dropped(data) => {
                self.drop_hover = false;
                self.install_dropped(data, context);
            }
            Message::LibraryMenu(item) => self.library_menu(&item, context),
            Message::ThemeMenu { id, item } => self.theme_menu(&id, &item, context),
            Message::RemoveClosed(result) => {
                if let Some((id, _)) = self.removing.take()
                    && result == ContentDialogResult::Primary
                {
                    self.remove_theme(&id, context);
                }
            }

            Message::AppMenu { process, item } => self.app_menu(&process, &item, context),
            Message::SetEnabled {
                process,
                id,
                enabled,
            } => self.set_enabled(&process, &id, enabled, context),
            Message::SetSetting {
                process,
                id,
                key,
                value,
            } => self.set_setting(&process, &id, key, value, context),
            Message::Expanded { key, expanded } => {
                if expanded {
                    self.expanded.insert(key);
                } else {
                    self.expanded.remove(&key);
                }
            }

            Message::About(open) => self.about_open = open,
        }
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        context.window_title("erebus");
        context.window_visuals(
            WindowVisuals::new()
                .backdrop(WindowBackdrop::Mica)
                .client_size(960.0, 640.0)
                .constraints(WindowConstraints {
                    min_width: Some(520.0),
                    min_height: Some(360.0),
                    ..Default::default()
                }),
        );
        // we can't change NavigationView's own breakpoints, and switching its display mode
        // while the pane opens leaves the content laid out at the old width. so we keep it in
        // one mode and just open or close the pane ourselves
        context.on_window_size(context.callback(Message::Resized));

        let page = match &self.selection {
            Selection::Library => self.library_page(context),
            Selection::App(process) => self.app_page(process, context),
        };

        let notices = self.notices.iter().rev().take(3).map(|notice| {
            let id = notice.id;
            KeyedView::new(
                id,
                InfoBar::new()
                    .margin(Thickness::new(24.0, 8.0, 24.0, 0.0))
                    .message(notice.text.clone())
                    .severity(notice.severity)
                    .is_open(true)
                    .on_closed(context.callback(move |()| Message::Dismiss(id))),
            )
        });

        let content = Grid::new()
            .rows([GridLength::Auto, GridLength::STAR])
            .margin(Thickness::new(0.0, TITLE_BAR_HEIGHT, 0.0, 0.0))
            .children((
                StackPanel::new().keyed_children(notices),
                // keyed by page, so one app's controls never get reused for another app
                Grid::new()
                    .grid_row(1)
                    .keyed_children([KeyedView::new(self.selection.tag(), page)]),
                self.remove_dialog(context),
                widgets::about_dialog(self.about_open, context.callback(|_| Message::About(false))),
            ));

        // win32 apps don't get NavigationView's automatic title bar padding so the pane
        // leaves room for it itself
        let navigation = NavigationView::new()
            .pane_display_mode(NavigationViewPaneDisplayMode::Left)
            .is_pane_toggle_button_visible(false)
            .is_pane_open(self.pane_open)
            .on_is_pane_open_changed(context.callback(Message::PaneOpen))
            .is_settings_visible(false)
            .is_back_button_visible(NavigationViewBackButtonVisible::Collapsed)
            .always_show_header(false)
            .open_pane_length(PANE_WIDTH)
            .on_selected_tag_changed(context.callback(Message::Navigate))
            .slots([
                SlotView::new(
                    NavigationViewSlot::PaneCustomContent,
                    Border::new().height(TITLE_BAR_HEIGHT),
                ),
                SlotView::collection(NavigationViewSlot::MenuItems, self.sidebar_items()),
                SlotView::new(NavigationViewSlot::Content, content),
                SlotView::new(NavigationViewSlot::PaneFooter, sidebar::about_item(context)),
            ]);

        let title_bar = TitleBar::new()
            .title("erebus")
            .is_pane_toggle_button_visible(true)
            .on_pane_toggle_requested(context.message(Message::TogglePane))
            .vertical_alignment(VerticalAlignment::Top);

        Grid::new().children((navigation, title_bar))
    }
}

impl Erebus {
    fn page_width(&self) -> f64 {
        let pane = if self.pane_open {
            PANE_WIDTH
        } else {
            COMPACT_PANE_WIDTH
        };
        // minus a pixel for the content area's border
        (self.window_width - pane - 1.0).clamp(0.0, MAX_PAGE_WIDTH)
    }

    fn wait_for_inbox(&self, context: &ComponentContext<Self>) {
        let inbox = self.inbox.clone();
        _ = context.spawn_background(move |cancel| Message::Inbox(Arc::new(inbox.wait(&cancel))));
    }

    fn notify(
        &mut self,
        text: String,
        severity: InfoBarSeverity,
        context: &ComponentContext<Self>,
    ) {
        let id = self.next_notice;
        self.next_notice += 1;
        self.notices.push(Notice { id, text, severity });

        _ = context.spawn_background(move |_| {
            std::thread::sleep(NOTICE_TIMEOUT);
            Message::Dismiss(id)
        });
    }

    fn notify_error(&mut self, text: String, context: &ComponentContext<Self>) {
        self.notify(text, InfoBarSeverity::Error, context);
    }
}
