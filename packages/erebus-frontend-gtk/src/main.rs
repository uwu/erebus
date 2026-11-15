mod logs;
mod ui;

use adw::prelude::*;

fn main() -> gtk::glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("network.uwu.Erebus")
        .build();

    app.connect_activate(ui::build);
    app.run()
}
