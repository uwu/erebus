mod components;
mod state;
mod style;
mod ui;

use zgui::prelude::*;
use zgui_ui::prelude::*;
use zgui_ui_tokens::prelude::*;

use crate::{state::AppState, ui::shell::MainWindowProps};

fn main() -> Result<(), zgui::Error> {
    #[cfg(windows)]
    erebus_frontend::inject::backend::load_system_library();

    app()
        .with_application_id("network.uwu.Erebus")
        .with_title("erebus")
        .with_transparent(true)
        .with_size(900.0, 640.0)
        .with_min_size(360.0, 400.0)
        .with_stylesheet(style::SHEET)
        .with_fonts(style::fonts())
        .with_decorations(Decorations::None)
        .run(|| {
            AppState::provide();

            view! {
                ThemeProvider(scheme = ColorScheme::Dark, dark = crate::style::theme()) {
                    TooltipProvider(delay = crate::components::tooltip::DELAY) {
                        Toaster {
                            MainWindow()
                        }
                    }
                }
            }
        })
}
