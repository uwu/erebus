use zgui::prelude::*;
use zgui::reactive::UnsyncCallback;
use zgui::style;
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;

use crate::components::octicons::PLUS;

style! { FloatingActionButtonStyle => r#"
    :scope:scope {
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.35), 0 1px 3px rgba(0, 0, 0, 0.25);
    }

    :scope .zui-icon {
        width: 18px;
        height: 18px;
    }
"# }

#[component]
pub fn FloatingActionButton(label: &'static str, on_click: UnsyncCallback<()>) -> impl IntoView {
    install_stylesheet(
        FloatingActionButtonStyle::CLASS,
        FloatingActionButtonStyle::CSS,
    );

    view! {
        Button(
            size = ButtonSize::Lg,
            class = FloatingActionButtonStyle::CLASS,
            on:click = move |_| on_click.run(())
        ) {
            Icon(icon = PLUS, label = "")
            text {{label}}
        }
    }
}
