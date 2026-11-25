//! We draw the window decorations ourselves. GNOME doesn't provide them for Wayland
//! clients, so using [`Decorations::None`] lets us use the same frame everywhere.

use std::{process::Command, sync::OnceLock};

use zgui::prelude::*;
use zgui::reactive::{RenderEffect, StoredValue};
use zgui::style;
use zgui_ui_icons::prelude::*;

use crate::components::octicons::{COPY, DASH, SQUARE, X};
use crate::components::tooltip::StyledTooltipProps;

/// The dialog scrim needs the same radius as the frame. Otherwise it darkens the transparent
/// corners outside the window.
const FRAMED_OVERLAYS: &str = "
    .zui-overlay-scrim.zui-overlay-scrim {
        border-radius: 10px;
    }
";

style! { FrameStyle => r#"
    :scope {
        position: relative;
        width: 100%;
        height: 100%;
    }

    :scope .frame {
        flex: 1 1 auto;
        min-width: 0;
        min-height: 0;
        overflow: hidden;
        background-color: var(--zui-color-sidebar);
    }

    :scope.floating .frame {
        border-radius: 10px;
        border: 1px solid color-mix(in oklab, var(--zui-color-foreground) 14%, transparent);
    }

    :scope.floating.backdrop .frame {
        border-color: color-mix(in oklab, var(--zui-color-foreground) 10%, transparent);
    }

    /* Keep the resize grips out of the close button's circle. They can overlap its padding. */
    :scope .grip {
        position: absolute;
    }

    :scope .resize-north {
        top: 0;
        left: 10px;
        right: 10px;
        height: 5px;
    }

    :scope .resize-south {
        bottom: 0;
        left: 10px;
        right: 10px;
        height: 5px;
    }

    :scope .resize-west {
        left: 0;
        top: 10px;
        bottom: 10px;
        width: 5px;
    }

    :scope .resize-east {
        right: 0;
        top: 10px;
        bottom: 10px;
        width: 5px;
    }

    :scope .resize-northwest {
        top: 0;
        left: 0;
        width: 10px;
        height: 10px;
    }

    :scope .resize-northeast {
        top: 0;
        right: 0;
        width: 10px;
        height: 10px;
    }

    :scope .resize-southwest {
        bottom: 0;
        left: 0;
        width: 10px;
        height: 10px;
    }

    :scope .resize-southeast {
        bottom: 0;
        right: 0;
        width: 10px;
        height: 10px;
    }

    :scope .titlebar {
        flex: 0 0 auto;
        height: 32px;
        align-items: center;
        gap: var(--zui-space-xs);
        background-color: var(--zui-color-background);
        color: var(--zui-color-foreground);
        position: relative;
        z-index: 0;
    }

    :scope .titlebar-content {
        flex: 1 1 auto;
        min-width: 0;
        padding: 0 6px;
        align-items: center;
        gap: var(--zui-space-xs);
    }

    :scope .titlebar.backdrop .titlebar-content {
        color: var(--zui-color-muted-foreground);
    }

    :scope .window-title {
        padding-left: var(--zui-space-xs);
        font-size: var(--zui-type-size-sm);
    }

    /* extend the hit areas to the titlebar's edges because of fitts law */
    :scope .window-controls {
        flex: 0 0 auto;
        align-self: stretch;
    }

    :scope .window-controls > .zui-tooltip__trigger {
        align-items: stretch;
    }

    :scope .window-controls.start {
        margin-right: var(--zui-space-xs);
    }

    :scope .window-control {
        display: flex;
        height: 100%;
        padding: 0 4px;
        align-items: center;
        justify-content: center;
        color: var(--zui-color-foreground);
    }

    :scope .window-controls.start > :first-child > .window-control {
        padding-left: 10px;
    }

    :scope .window-controls.end > :last-child > .window-control {
        padding-right: 10px;
    }

    :scope .circle {
        display: flex;
        width: 20px;
        height: 20px;
        border-radius: 10px;
        align-items: center;
        justify-content: center;
        background-color: color-mix(in oklab, var(--zui-color-foreground) 9%, transparent);
    }

    :scope .window-control:hover .circle {
        background-color: color-mix(in oklab, var(--zui-color-foreground) 16%, transparent);
    }

    :scope .window-control:active .circle {
        background-color: color-mix(in oklab, var(--zui-color-foreground) 24%, transparent);
    }

    :scope .window-control:focus-visible {
        outline: none;
    }

    :scope .window-control:focus-visible .circle {
        box-shadow: 0 0 0 2px var(--zui-color-ring);
    }

    :scope .titlebar.backdrop .window-control {
        color: var(--zui-color-muted-foreground);
    }

    :scope .circle > .zui-icon {
        width: 12px;
        height: 12px;
    }
"# }

/// Drop the border and rounded corners when maximized or fullscreen.
#[component]
pub fn WindowFrame(children: Children) -> impl IntoView {
    let window = use_window();
    let floating = {
        let window = window.clone();
        Signal::derive_local(move || {
            !window.maximized().get() && window.fullscreen().get().is_none()
        })
    };
    let focused = window.focused();

    let overlays = RenderEffect::new(move |_| {
        install_stylesheet(
            "erebus-window-frame",
            if floating.get() { FRAMED_OVERLAYS } else { "" },
        );
    });
    StoredValue::new_local(overlays);
    install_stylesheet(FrameStyle::CLASS, FrameStyle::CSS);

    view! {
        column(
            class = FrameStyle::CLASS,
            class:floating = move || floating.get(),
            class:backdrop = move || !focused.get()
        ) {
            column(class = "frame") {{children.into_view_once()}}
            if move || floating.get() {
                Grip(edge = ResizeEdge::North, class = "grip resize-north")
                Grip(edge = ResizeEdge::South, class = "grip resize-south")
                Grip(edge = ResizeEdge::West, class = "grip resize-west")
                Grip(edge = ResizeEdge::East, class = "grip resize-east")
                Grip(edge = ResizeEdge::NorthWest, class = "grip resize-northwest")
                Grip(edge = ResizeEdge::NorthEast, class = "grip resize-northeast")
                Grip(edge = ResizeEdge::SouthWest, class = "grip resize-southwest")
                Grip(edge = ResizeEdge::SouthEast, class = "grip resize-southeast")
            } else {}
        }
    }
}

#[component]
fn Grip(edge: ResizeEdge, class: &'static str) -> impl IntoView {
    let window = use_window();
    let cursor = match edge {
        ResizeEdge::North | ResizeEdge::South => CursorStyle::ResizeNorthSouth,
        ResizeEdge::East | ResizeEdge::West => CursorStyle::ResizeEastWest,
        ResizeEdge::NorthWest | ResizeEdge::SouthEast => CursorStyle::ResizeNorthWestSouthEast,
        _ => CursorStyle::ResizeNorthEastSouthWest,
    };

    view! {
        box(
            class = class,
            on:pointer_down = window.resize_drag_handler(edge),
            on:pointer_enter = {
                let window = window.clone();
                move |_| window.set_cursor(cursor)
            },
            on:pointer_leave = {
                let window = window.clone();
                move |_| window.set_cursor(CursorStyle::Default)
            }
        )
    }
}

#[component]
pub fn Titlebar(children: Children) -> impl IntoView {
    let window = use_window();
    let focused = window.focused();

    view! {
        row(
            class = "titlebar",
            class:backdrop = move || !focused.get(),
            on:pointer_down = window.move_drag_handler()
        ) {
            WindowControls(side = Side::Start)
            row(class = "titlebar-content") {{children.into_view_once()}}
            WindowControls(side = Side::End)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Start,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WindowButton {
    Minimize,
    Maximize,
    Close,
}

#[component]
pub fn WindowControls(side: Side) -> impl IntoView {
    let (start, end) = button_layout();
    let buttons = match side {
        Side::Start => start,
        Side::End => end,
    };
    if buttons.is_empty() {
        return AnyView::new(());
    }

    let window = use_window();
    let buttons = buttons
        .iter()
        .map(|button| view! { WindowControl(button = *button) }.into_view())
        .collect::<Vec<_>>();

    AnyView::new(view! {
        row(
            class = "window-controls",
            class:start = side == Side::Start,
            class:end = side == Side::End,
            on:pointer_down = window.no_drag_handler()
        ) {{buttons}}
    })
}

#[component]
fn WindowControl(button: WindowButton) -> impl IntoView {
    let window = use_window();
    let (label, icon): (&'static str, Box<dyn Fn() -> IconData>) = match button {
        WindowButton::Minimize => ("Minimize", Box::new(|| DASH)),
        WindowButton::Maximize => {
            let maximized = window.maximized();
            (
                "Maximize",
                Box::new(move || if maximized.get() { COPY } else { SQUARE }),
            )
        }
        WindowButton::Close => ("Close", Box::new(|| X)),
    };

    view! {
        StyledTooltip(label = label) {
            control(
                class = "window-control",
                tabindex = Focus::Sequential,
                a11y:role = zgui::vocab::Role::Button,
                a11y:label = label,
                on:click = move |_| match button {
                    WindowButton::Minimize => window.minimize(),
                    WindowButton::Maximize => window.toggle_maximized(),
                    WindowButton::Close => window.close(),
                }
            ) {
                box(class = "circle") {
                    Icon(icon = Signal::derive_local(icon), label = "")
                }
            }
        }
    }
}

/// Respect GNOME's button layout when available. Elsewhere we put all three buttons on the
/// right.
fn button_layout() -> &'static (Vec<WindowButton>, Vec<WindowButton>) {
    static LAYOUT: OnceLock<(Vec<WindowButton>, Vec<WindowButton>)> = OnceLock::new();
    LAYOUT.get_or_init(|| {
        let gnome = std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| {
            desktop
                .split(':')
                .any(|name| name.eq_ignore_ascii_case("gnome"))
        });
        let setting = gnome
            .then(|| {
                Command::new("gsettings")
                    .args(["get", "org.gnome.desktop.wm.preferences", "button-layout"])
                    .output()
                    .ok()
                    .filter(|output| output.status.success())
                    .and_then(|output| String::from_utf8(output.stdout).ok())
            })
            .flatten();
        setting.as_deref().map_or_else(
            || {
                let all = vec![
                    WindowButton::Minimize,
                    WindowButton::Maximize,
                    WindowButton::Close,
                ];
                (Vec::new(), all)
            },
            parse_layout,
        )
    })
}

/// The colon separates the two sides, e.g. `'appmenu:minimize,maximize,close'`. A missing colon
/// puts everything at the start, matching GTK.
fn parse_layout(layout: &str) -> (Vec<WindowButton>, Vec<WindowButton>) {
    let layout = layout.trim().trim_matches('\'');
    let (start, end) = layout.split_once(':').unwrap_or((layout, ""));
    let buttons = |names: &str| {
        names
            .split(',')
            .filter_map(|name| match name.trim() {
                "minimize" => Some(WindowButton::Minimize),
                "maximize" => Some(WindowButton::Maximize),
                "close" => Some(WindowButton::Close),
                _ => None,
            })
            .collect()
    };
    (buttons(start), buttons(end))
}
