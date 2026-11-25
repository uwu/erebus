use std::time::Duration;

use zgui::prelude::*;
use zgui::reactive::RwSignal;
use zgui::style;
use zgui_ui::prelude::*;
use zgui_ui_primitives::Placement;

pub const DELAY: Duration = Duration::from_millis(500);

style! { TooltipStyle => r#"
    :scope:scope {
        --zui-surface-enter-duration: 150ms;
        --zui-surface-exit-duration: 100ms;
        --zui-surface-enter-ease: cubic-bezier(0.16, 1, 0.3, 1);
        --zui-surface-exit-ease: var(--zui-motion-ease-standard);
        --zui-surface-shadow: 0 4px 12px rgba(0, 0, 0, 0.35);
    }
"# }

#[component]
pub fn StyledTooltip(#[prop(into)] label: String, children: Children) -> impl IntoView {
    let open = RwSignal::new_local(false);
    let pressed = RwSignal::new_local(false);

    install_stylesheet(TooltipStyle::CLASS, TooltipStyle::CSS);

    let shown = Binding::controlled(
        Signal::derive_local(move || open.get() && !pressed.get()),
        move |now| open.set(now),
    );

    view! {
        Tooltip(open = shown) {
            TooltipTrigger(
                on:pointer_down = move |_| pressed.set(true),
                on:pointer_leave = move |_| pressed.set(false)
            ) {{children.into_view_once()}}
            TooltipContent(placement = Placement::BOTTOM, arrow = false, class = TooltipStyle::CLASS) {{label.clone()}}
        }
    }
}
