//! The main window lives here.

mod header;
mod sidebar;

pub use header::{
    AppMenuItemsProps, HeaderProps, HeaderStyle, LibraryMenuItemsProps, MainMenuProps,
};

use zgui::prelude::*;
use zgui::reactive::UnsyncCallback;
use zgui::style;
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;

use crate::{
    components::{
        fab::FloatingActionButtonProps,
        octicons::PLUS,
        tooltip::StyledTooltipProps,
        window_frame::{TitlebarProps, WindowFrameProps},
    },
    state::{AppEntry, AppState, Nav, Selection},
    ui::{
        dialogs::{
            about::AboutDialogProps, confirm::ConfirmDialogProps, inject::InjectDialogProps,
        },
        pages::{app::AppPageProps, library::LibraryPageProps},
    },
};

use sidebar::{AppEntryRowProps, LibraryEntryProps};

style! { ShellStyle => r#"
    /* Override the provider's full height so the shell fits below the titlebar. Repeating :scope
       gives this rule enough specificity. */
    :scope:scope {
        flex: 1 1 auto;
        min-height: 0;
        height: auto;
        width: 100%;
    }

    /* The sidebar's built-in transition would animate window resizing across the breakpoint. */
    :scope .zui-sidebar, :scope .zui-sidebar__container, :scope .zui-sidebar__inset {
        transition: none;
    }

    :scope.narrow {
        --zui-sidebar-width: 100%;
    }

    :scope.list > .content {
        display: none;
    }

    /* The window border already covers this edge when the sidebar fills the width. */
    :scope.narrow[data-side] .zui-sidebar__container.zui-sidebar__container {
        border-right: none;
    }

    :scope .content {
        min-width: 0;
        min-height: 0;
        overflow: hidden;
    }

    /* AppState::nav sets push/pop only for navigation, not for window resizing. */
    :scope.push > .content {
        animation: shell-push var(--zui-motion-duration-slower) cubic-bezier(0.32, 0.72, 0, 1);
    }

    :scope.pop .zui-sidebar__container {
        animation: shell-pop var(--zui-motion-duration-slower) cubic-bezier(0.32, 0.72, 0, 1);
    }

    @keyframes shell-push {
        from {
            transform: translateX(64px);
            opacity: 0;
        }
    }

    @keyframes shell-pop {
        from {
            transform: translateX(-64px);
            opacity: 0;
        }
    }

    :scope .entry-menu {
        width: 100%;
    }

    /* In narrow windows, put the inject button in the same corner as the library's action
       button. */
    :scope .inject-button {
        position: absolute;
        left: var(--zui-space-lg);
        bottom: var(--zui-space-lg);
    }

    :scope.narrow .inject-button {
        left: auto;
        right: var(--zui-space-lg);
    }

    /* room under the last app, so the button never covers it */
    :scope .zui-sidebar__content {
        padding-bottom: calc(44px + var(--zui-space-lg) * 2);
    }

    /* as wide as an app's icon, so the library's text lines up with the apps' below it */
    :scope .library-icon {
        display: flex;
        flex: 0 0 auto;
        width: 24px;
        height: 24px;
        align-items: center;
        justify-content: center;
    }

    :scope .sidebar-empty {
        flex: none;
        border-width: 1px;
        padding: var(--zui-space-lg);
        gap: var(--zui-space-md);
    }

    :scope .two-lines {
        flex: 1 1 auto;
        min-width: 0;
    }

    :scope .entry-title {
        font-size: var(--zui-type-size-sm);
    }

    :scope .entry-subtitle {
        font-size: var(--zui-type-size-xs);
        color: var(--zui-color-muted-foreground);
    }

    :scope .status-dot {
        flex: 0 0 auto;
        width: 8px;
        height: 8px;
        border-radius: 4px;
        background-color: var(--zui-color-border);
    }

    :scope .status-dot.running {
        background-color: var(--zui-color-success);
    }
"# }

// the sidebar's colours, continued up into the titlebar
style! { PaneStyle => r#"
    :scope {
        position: absolute;
        top: 0;
        bottom: 0;
        left: 0;
        z-index: -1;
        width: 256px;
        border-right: 1px solid var(--zui-color-sidebar-border);
        background-color: var(--zui-color-sidebar);
    }

    :scope.narrow {
        width: 100%;
        border-right: none;
    }

    /* Fade the sidebar colour out of the titlebar when navigating to a page. */
    :scope.narrow.page {
        opacity: 0;
    }

    :scope.push {
        animation: pane-out var(--zui-motion-duration-slower) var(--zui-motion-ease-standard);
    }

    :scope.pop {
        animation: pane-in var(--zui-motion-duration-slower) var(--zui-motion-ease-standard);
    }

    @keyframes pane-out {
        from {
            opacity: 1;
        }
    }

    @keyframes pane-in {
        from {
            opacity: 0;
        }
    }
"# }

style! { WordmarkStyle => r#"
    :scope {
        padding: 0 var(--zui-space-xs);
        font-family: "Geist Serif";
        font-size: var(--zui-type-size-md);
        transition: color var(--zui-motion-duration-fast) var(--zui-motion-ease-standard);
    }

    :scope:hover {
        color: var(--zui-color-primary);
    }

    :scope:focus-visible {
        color: var(--zui-color-primary);
        outline: 2px solid var(--zui-color-ring);
        outline-offset: 2px;
        border-radius: var(--zui-radius-sm);
    }
"# }

#[component]
pub fn MainWindow() -> impl IntoView {
    let app = AppState::get();
    let window = use_window();
    install_stylesheet(PaneStyle::CLASS, PaneStyle::CSS);
    install_stylesheet(WordmarkStyle::CLASS, WordmarkStyle::CSS);

    view! {
        WindowFrame {
            Titlebar {
                box(
                    class = PaneStyle::CLASS,
                    class:narrow = move || app.narrow.get(),
                    class:page = move || app.narrow.get() && app.showing_page.get(),
                    class:push = move || app.nav.get() == Some(Nav::Push),
                    class:pop = move || app.nav.get() == Some(Nav::Pop)
                )
                StyledTooltip(label = "About erebus") {
                    control(
                        class = WordmarkStyle::CLASS,
                        tabindex = Focus::Sequential,
                        a11y:role = zgui::vocab::Role::Button,
                        a11y:label = "About erebus",
                        on:pointer_down = window.no_drag_handler(),
                        on:click = move |_| app.about_open.set(true)
                    ) {
                        "erebus"
                    }
                }
            }
            Shell()
        }
        InjectDialog()
        ConfirmDialog()
        AboutDialog()
    }
}

#[component]
pub fn Shell() -> impl IntoView {
    let app = AppState::get();

    let page = move || match app.selection.get() {
        Selection::Library => AnyView::new(view! { LibraryPage() }),
        Selection::App(app_id) => AnyView::new(view! { AppPage(app_id = app_id) }),
    };
    install_stylesheet(ShellStyle::CLASS, ShellStyle::CSS);

    view! {
        SidebarProvider(
            open = Binding::controlled(
                Signal::derive_local(move || !app.narrow.get() || !app.showing_page.get()),
                |_| {}
            ),
            collapse = SidebarCollapse::Offcanvas,
            shortcut = false,
            class = ShellStyle::CLASS,
            class:narrow = move || app.narrow.get(),
            class:page = move || app.narrow.get() && app.showing_page.get(),
            class:list = move || app.narrow.get() && !app.showing_page.get(),
            class:push = move || app.nav.get() == Some(Nav::Push),
            class:pop = move || app.nav.get() == Some(Nav::Pop)
        ) {
            Sidebar(label = "erebus") {
                SidebarContent {
                    SidebarGroup {
                        SidebarGroupContent {
                            SidebarMenu {LibraryEntry()}
                        }
                    }
                    SidebarGroup {
                        SidebarGroupLabel {"Apps"}
                        SidebarGroupContent {
                            SidebarMenu {
                                for entry in move || app.apps(), key = |entry: &AppEntry| entry.clone() {
                                    AppEntryRow(entry = entry)
                                }
                            }
                            if move || app.apps().is_empty() {
                                Empty(class = "sidebar-empty") {
                                    EmptyHeader {
                                        EmptyTitle {"No apps yet"}
                                        EmptyDescription {"Inject into an app to theme it."}
                                    }
                                    EmptyContent {
                                        Button(
                                            variant = ButtonVariant::Outline,
                                            size = ButtonSize::Sm,
                                            on:click = move |_| app.inject_open.set(true)
                                        ) {
                                            Icon(icon = PLUS, label = "")
                                            "Inject erebus"
                                        }
                                    }
                                }
                            } else {}
                        }
                    }
                }
                row(class = "inject-button") {
                    FloatingActionButton(label = "Inject", on_click = UnsyncCallback::new(move |()| app.inject_open.set(true)))
                }
            }
            SidebarInset(class = "content") {{page}}
        }
    }
}
