use zgui::prelude::*;
use zgui::reactive::{RenderEffect, StoredValue, UnsyncCallback};
use zgui::style;
use zgui::view::{ListenerGuard, events};
use zgui::vocab::{Key, ListenerOptions, NamedKey};
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;

use crate::{
    components::{
        octicons::{CHEVRON_LEFT, KEBAB_HORIZONTAL},
        tooltip::StyledTooltipProps,
    },
    state::AppState,
    ui::logs,
};

style! { pub HeaderStyle => r#"
    :scope {
        flex: 0 0 auto;
        align-items: center;
        gap: var(--zui-space-sm);
        min-height: 60px;
        padding: var(--zui-space-sm) var(--zui-space-xl);
        border-bottom: 1px solid var(--zui-color-border);
    }

    /* Keep the back button and menu equally inset in narrow windows. */
    :scope.narrow {
        padding-left: var(--zui-space-md);
        padding-right: var(--zui-space-md);
    }

    :scope .actions {
        flex: 0 0 auto;
        align-items: center;
        gap: var(--zui-space-xs);
    }

    :scope .titles {
        flex: 1 1 auto;
        min-width: 0;
        gap: 2px;
    }

    :scope .title {
        font-size: var(--zui-type-size-lg);
        font-weight: var(--zui-type-weight-semibold);
    }

    :scope .subtitle {
        font-size: var(--zui-type-size-xs);
        color: var(--zui-color-muted-foreground);
    }
"# }

#[component]
pub fn Header(
    #[prop(into)] title: Signal<String, zgui::reactive::LocalStorage>,
    #[prop(into)] subtitle: Signal<String, zgui::reactive::LocalStorage>,
    children: Children,
) -> impl IntoView {
    let app = AppState::get();
    install_stylesheet(HeaderStyle::CLASS, HeaderStyle::CSS);

    // Listen at the window root so Escape can return to the sidebar regardless of focus. Menus,
    // dialogs and the search field get to consume it first.
    let element = NodeRef::new();
    let held = StoredValue::new_local(None::<ListenerGuard>);
    let watching = RenderEffect::new(move |_| {
        if element.get().is_none() || held.with_value(Option::is_some) {
            return;
        }
        let Some(root) = element.window_root() else {
            return;
        };
        held.set_value(root.listen(
            events::KEY_DOWN,
            ListenerOptions::DEFAULT,
            move |ev: &mut EventCx<'_, events::KeyDown>| {
                if ev.key == Key::Named(NamedKey::Escape)
                    && app.narrow.get_untracked()
                    && app.showing_page.get_untracked()
                {
                    ev.stop_propagation();
                    app.showing_page.set(false);
                }
            },
        ));
    });
    on_cleanup_local(move || drop(watching));

    view! {
        row(class = HeaderStyle::CLASS, class:narrow = move || app.narrow.get(), node_ref = element) {
            if move || app.narrow.get() {
                StyledTooltip(label = "Back") {
                    Button(
                        variant = ButtonVariant::Ghost,
                        size = ButtonSize::IconSm,
                        class = "back",
                        a11y:label = "Back",
                        on:click = move |_| app.showing_page.set(false)
                    ) {
                        Icon(icon = CHEVRON_LEFT, label = "")
                    }
                }
            } else {}
            column(class = "titles") {
                text(class = "title") {{move || title.get()}}
                text(class = "subtitle") {{move || subtitle.get()}}
            }
            row(class = "actions") {{children.into_view_once()}}
        }
    }
}

/// Page-specific actions go before the shared app actions.
#[component]
pub fn MainMenu(children: ChildrenFn) -> impl IntoView {
    let app = AppState::get();

    view! {
        StyledTooltip(label = "Main Menu") {
            DropdownMenu {
                DropdownMenuTrigger(
                    variant = ButtonVariant::Ghost,
                    size = ButtonSize::IconSm,
                    a11y:label = "Main Menu"
                ) {
                    Icon(icon = KEBAB_HORIZONTAL, label = "")
                }
                DropdownMenuContent {
                    {children.view()}
                    MenuSeparator()
                    MenuItem(on_select = UnsyncCallback::new(move |()| app.about_open.set(true))) {
                        "About erebus"
                    }
                }
            }
        }
    }
}

/// Shared by the page menu and the sidebar's context menu.
#[component]
pub fn LibraryMenuItems() -> impl IntoView {
    let app = AppState::get();
    view! {
        MenuItem(on_select = UnsyncCallback::new(move |()| app.open_config_folder())) {
            "Open Config Folder"
        }
        MenuItem(on_select = UnsyncCallback::new(move |()| app.reload_library())) {
            "Reload Library"
        }
    }
}

/// Shared by the page menu and the sidebar's context menu.
#[component]
pub fn AppMenuItems(#[prop(into)] app_id: String) -> impl IntoView {
    let app = AppState::get();
    let logs = {
        let app_id = app_id.clone();
        UnsyncCallback::new(move |()| logs::open(app, &app_id))
    };
    let removable = {
        let app_id = app_id.clone();
        move || app.is_removable(&app_id)
    };
    let remove = UnsyncCallback::new(move |()| {
        let app_id = app_id.clone();
        app.confirm_remove_erebus(&app_id.clone(), move || app.forget_app(&app_id));
    });

    view! {
        MenuItem(on_select = logs) {"View Logs"}
        if move || removable() {
            MenuItem(destructive = true, on_select = remove) {"Remove erebus"}
        } else {}
    }
}
