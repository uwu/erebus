use zgui::prelude::*;
use zgui_ui::prelude::*;

use crate::state::AppState;

const REPO: &str = "https://github.com/uwu/erebus";

#[component]
pub fn AboutDialog() -> impl IntoView {
    let app = AppState::get();

    view! {
        Dialog(open = app.about_open) {
            DialogContent {
                DialogHeader {
                    DialogTitle {"erebus-frontend-zgui"}
                    DialogDescription {"Manage themes for GPUI apps running with erebus"}
                }
                column(style:gap = "var(--zui-space-xs)".to_owned()) {
                    text {{format!("Version {}", env!("CARGO_PKG_VERSION"))}}
                    text {"uwu.network"}
                    DialogDescription {"© 2026 Tropical · MIT License"}
                }
                DialogFooter {
                    Button(
                        variant = ButtonVariant::Outline,
                        on:click = move |_| app.open_external(&format!("{REPO}/issues"))
                    ) {
                        "Report an Issue"
                    }
                    Button(on:click = move |_| app.open_external(REPO)) {"Website"}
                }
            }
        }
    }
}
