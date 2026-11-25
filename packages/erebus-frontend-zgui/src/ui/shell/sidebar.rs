use erebus_frontend::i18n;
use zgui::prelude::*;
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;

use super::header::{AppMenuItemsProps, LibraryMenuItemsProps};
use crate::{
    components::octicons::PAINTBRUSH,
    state::{AppEntry, AppState, Selection},
    ui::app_icon::AppIconProps,
};

#[component]
pub fn LibraryEntry() -> impl IntoView {
    let app = AppState::get();
    let themes = move || {
        i18n::theme_count(
            app.library
                .with(|library| library.config_store.themes.len()),
        )
    };

    view! {
        SidebarMenuItem {
            ContextMenu {
                ContextMenuTrigger(class = "entry-menu") {
                    SidebarMenuButton(
                        size = SidebarMenuSize::Lg,
                        active = Signal::derive_local(move || {
                            !app.narrow.get() && app.selection.get() == Selection::Library
                        }),
                        on:click = move |_| app.open(Selection::Library)
                    ) {
                        box(class = "library-icon") {Icon(icon = PAINTBRUSH, label = "")}
                        TwoLines(title = "Theme Library", subtitle = Signal::derive_local(themes))
                    }
                }
                ContextMenuContent {LibraryMenuItems()}
            }
        }
    }
}

#[component]
pub fn AppEntryRow(entry: AppEntry) -> impl IntoView {
    let app = AppState::get();
    let AppEntry { name, app_id, icon } = entry;

    let selection = Selection::App(app_id.clone());
    let active = {
        let selection = selection.clone();
        // In a narrow window the selected page is hidden while the sidebar is open, so don't
        // highlight its row.
        Signal::derive_local(move || !app.narrow.get() && app.selection.get() == selection)
    };
    let status = {
        let app_id = app_id.clone();
        Signal::derive_local(move || i18n::status(app.instances(&app_id)))
    };
    let running = {
        let app_id = app_id.clone();
        move || app.instances(&app_id) > 0
    };

    view! {
        SidebarMenuItem {
            ContextMenu {
                ContextMenuTrigger(class = "entry-menu") {
                    SidebarMenuButton(
                        size = SidebarMenuSize::Lg,
                        active = active,
                        on:click = move |_| app.open(selection.clone())
                    ) {
                        AppIcon(icon = icon, size = 24.0)
                        TwoLines(title = name, subtitle = status)
                        box(class = "status-dot", class:running = running)
                    }
                }
                ContextMenuContent {AppMenuItems(app_id = app_id.clone())}
            }
        }
    }
}

#[component]
fn TwoLines(
    #[prop(into)] title: String,
    #[prop(into)] subtitle: Signal<String, zgui::reactive::LocalStorage>,
) -> impl IntoView {
    view! {
        column(class = "two-lines") {
            text(class = "entry-title") {{title}}
            text(class = "entry-subtitle") {{move || subtitle.get()}}
        }
    }
}
