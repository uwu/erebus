use zgui::prelude::*;
use zgui::style;
use zgui_ui::prelude::*;

use crate::state::AppState;

// https://uwu.network/uwus/3.txt
const UWU_3: &str = r"  __  ___      ____  __
 / / / / | /| / / / / /
/ /_/ /| |/ |/ / /_/ /
\__,_/ |__/|__/\__,_/";

style! { AboutStyle => r#"
    :scope .heading {
        align-items: center;
        gap: var(--zui-space-md);
    }

    :scope .network {
        width: 100%;
        height: auto;
        padding: var(--zui-space-xl);
        background-color: var(--zui-color-card);
        border-color: var(--zui-color-border);
        border-radius: var(--zui-radius-lg);
        color: var(--zui-color-primary);
    }

    :scope .network:hover {
        background-color: var(--zui-color-accent);
        border-color: var(--zui-color-primary);
        color: var(--zui-color-primary);
    }

    :scope .network-content {
        align-items: center;
        gap: var(--zui-space-lg);
    }

    :scope .wordmark {
        font-family: var(--zui-type-family-mono);
        font-size: 16px;
        font-weight: 400;
        line-height: 20px;
        white-space: pre;
    }
"# }

const REPO: &str = "https://github.com/uwu/erebus";

#[component]
pub fn AboutDialog() -> impl IntoView {
    let app = AppState::get();
    install_stylesheet(AboutStyle::CLASS, AboutStyle::CSS);

    view! {
        Dialog(open = app.about_open) {
            DialogContent(class = AboutStyle::CLASS) {
                DialogHeader {
                    row(class = "heading") {
                        DialogTitle {{env!("CARGO_PKG_NAME")}}
                        Badge(variant = BadgeVariant::Secondary) {
                            {{env!("CARGO_PKG_VERSION")}}
                        }
                    }
                }
                DialogDescription {"Manage themes for GPUI apps running with erebus"}
                Button(
                    variant = ButtonVariant::Outline,
                    class = "network",
                    a11y:label = "Visit uwu.network",
                    on:click = move |_| app.open_external("https://uwu.network")
                ) {
                    column(class = "network-content") {
                        text(class = "wordmark", a11y:hidden = true) {{UWU_3}}
                        text {"uwu.network ↗"}
                    }
                }
                DialogDescription {"© 2026 uwu.network. Licensed under the MIT License."}
                DialogFooter {
                    Button(
                        variant = ButtonVariant::Outline,
                        on:click = move |_| app.open_external(&format!("{REPO}/issues"))
                    ) {
                        "Report an Issue"
                    }
                    Button(on:click = move |_| app.open_external(REPO)) {"GitHub"}
                }
            }
        }
    }
}
