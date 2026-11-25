//! The window that shows the logs from an app's backends.

use std::rc::Rc;

use log::Level;
use zgui::prelude::*;
use zgui::reactive::{RenderEffect, RwSignal, StoredValue, Trigger};
use zgui::style;
use zgui::view::{ScrollBehavior, ScrollTarget};
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;
use zgui_ui_tokens::prelude::*;

use crate::{
    components::{
        octicons::TRASH,
        tooltip::StyledTooltipProps,
        window_frame::{self, TitlebarProps, WindowFrameProps},
    },
    state::{AppState, Line, LogBuffer},
    ui::shell::HeaderStyle,
};

style! { LogsStyle => r#"
    :scope {
        height: 100%;
    }

    :scope .log-scroll {
        flex: 1 1 auto;
        min-height: 0;
    }

    :scope .lines {
        padding: var(--zui-space-md);
        font-family: var(--zui-type-family-mono);
        font-size: var(--zui-type-size-sm);
    }

    :scope .line {
        gap: 1ch;
        flex-wrap: wrap;
    }

    :scope .line.error {
        color: var(--zui-color-destructive);
    }

    :scope .line.warn {
        color: var(--zui-color-warning);
    }

    :scope .dim {
        color: var(--zui-color-muted-foreground);
    }

    :scope .message {
        white-space: pre-wrap;
    }

    :scope .log-end {
        height: 1px;
    }
"# }

const ALL_LEVELS: [Level; 5] = [
    Level::Error,
    Level::Warn,
    Level::Info,
    Level::Debug,
    Level::Trace,
];

fn label(level: Level) -> &'static str {
    match level {
        Level::Error => "Errors",
        Level::Warn => "Warnings",
        Level::Info => "Info",
        Level::Debug => "Debug",
        Level::Trace => "Trace",
    }
}

pub fn open(app: AppState, app_id: &str) {
    let buffer = app
        .runtime
        .try_update_untracked(|runtime| runtime.logs(app_id));
    let Some(buffer) = buffer else {
        return;
    };

    let app_id = app_id.to_owned();
    app.windows().open(
        WindowOptions::new(format!("{app_id} logs"))
            .with_size(760.0, 480.0)
            .with_decorations(Decorations::None)
            .with_transparent(true)
            .with_stylesheet(crate::style::SHEET),
        move || {
            let (app_id, buffer) = (app_id.clone(), buffer.clone());
            view! {
                ThemeProvider(scheme = ColorScheme::Dark, dark = crate::style::theme()) {
                    TooltipProvider(delay = crate::components::tooltip::DELAY) {
                        WindowFrame {
                            Logs(app = app, app_id = app_id, buffer = buffer)
                        }
                    }
                }
            }
        },
    );
}

#[component]
fn Logs(app: AppState, app_id: String, buffer: LogBuffer) -> impl IntoView {
    let max_level = RwSignal::new_local(buffer.max_level());
    let cleared = Trigger::new();

    let level = Binding::controlled(
        Signal::derive_local(move || max_level.get().as_str().to_owned()),
        {
            let buffer = buffer.clone();
            move |value: String| {
                if let Some(level) = ALL_LEVELS.into_iter().find(|level| level.as_str() == value) {
                    buffer.set_max_level(level);
                    max_level.set(level);
                }
            }
        },
    );

    let lines = {
        let buffer = buffer.clone();
        move || {
            app.logs_changed.track();
            cleared.track();
            buffer.visible(max_level.get())
        }
    };

    let clear = {
        let buffer = buffer.clone();
        move |_: &mut EventCx<'_, zgui::view::events::Click>| {
            buffer.clear();
            cleared.notify();
        }
    };

    // follows new records as they come in, once they've been laid out
    let end = NodeRef::new();
    let timers = Timers::current();
    let pending = StoredValue::new_local(None);
    let follow = RenderEffect::new(move |_| {
        app.logs_changed.track();
        cleared.track();
        max_level.track();
        if let Some(timers) = &timers {
            pending.set_value(Some(timers.request_frame(move |_| {
                end.scroll_to(ScrollTarget::IntoViewEnd, ScrollBehavior::Instant);
            })));
        }
    });
    StoredValue::new_local(follow);
    install_stylesheet(LogsStyle::CLASS, LogsStyle::CSS);
    install_stylesheet(HeaderStyle::CLASS, HeaderStyle::CSS);
    view! {
        column(class = LogsStyle::CLASS) {
            Titlebar {
                text(class = "window-title") {"Logs"}
            }
            row(class = HeaderStyle::CLASS) {
                column(class = "titles") {
                    text(class = "title") {{app_id}}
                    text(class = "subtitle") {"Messages from erebus in this app"}
                }
                Select(value = level) {
                    SelectTrigger(size = SelectTriggerSize::Sm, a11y:label = "Most Verbose Level Shown") {
                        SelectValue()
                    }
                    SelectContent {
                        {ALL_LEVELS
                            .into_iter()
                            .map(|level| view! { SelectItem(value = level.as_str()) {{label(level)}} }.into_view())
                            .collect::<Vec<_>>()}
                    }
                }
                StyledTooltip(label = "Clear Logs") {
                    Button(
                        variant = ButtonVariant::Ghost,
                        size = ButtonSize::Icon,
                        a11y:label = "Clear Logs",
                        on:click = clear
                    ) {
                        Icon(icon = TRASH, label = "")
                    }
                }
            }
            scroll(class = "log-scroll") {
                column(class = "lines") {
                    for line in move || lines(), key = |line: &Rc<Line>| line.id {
                        row(
                            class = "line",
                            class:error = line.level == Level::Error,
                            class:warn = line.level == Level::Warn
                        ) {
                            text(class = "dim") {{line.origin.clone()}}
                            text(class = "level") {{format!("{:<5}", line.level.as_str())}}
                            text(class = "dim") {{line.target.clone()}}
                            text(class = "message") {{line.message.clone()}}
                        }
                    }
                    box(node_ref = end, class = "log-end")
                }
            }
        }
    }
}
