//! UI and business logic for application logs.

use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use chrono::{DateTime, Local};
use erebus_frontend::apps::LogStorage;
use erebus_ipc::protocol::LogRecord;
use windows_reactor::*;

use crate::ui::widgets::icon_label;

/// how many lines each app hangs on to
const KEPT: usize = 2_000;
/// how many lines the window shows at once
const SHOWN: usize = 500;
/// how often a waiting task checks whether it's been cancelled
const CANCEL_POLL: Duration = Duration::from_millis(250);

const ALL_LEVELS: [log::Level; 5] = [
    log::Level::Error,
    log::Level::Warn,
    log::Level::Info,
    log::Level::Debug,
    log::Level::Trace,
];

fn label(level: log::Level) -> &'static str {
    match level {
        log::Level::Error => "Errors",
        log::Level::Warn => "Warnings",
        log::Level::Info => "Info",
        log::Level::Debug => "Debug",
        log::Level::Trace => "Trace",
    }
}

#[derive(Clone)]
pub struct LogLine {
    level: log::Level,
    text: String,
}

#[derive(Default)]
struct Lines {
    lines: VecDeque<LogLine>,
    /// goes up on every change, so a window knows when it needs to redraw
    revision: u64,
}

/// One app's logs. Its log windows share it and wait for changes on background threads.
#[derive(Clone, Default)]
pub struct Logs(Arc<(Mutex<Lines>, Condvar)>);

impl PartialEq for Logs {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl LogStorage for Logs {
    fn push(&self, pid: u32, record: &LogRecord) {
        let time = DateTime::from_timestamp_millis(record.timestamp as i64)
            .map(|time| {
                time.with_timezone(&Local)
                    .format("%H:%M:%S%.3f")
                    .to_string()
            })
            .unwrap_or_default();

        let mut lines = self.lock();
        lines.lines.push_back(LogLine {
            level: record.level,
            text: format!(
                "{time} [{pid}] {:<5} {}: {}",
                record.level, record.target, record.message
            ),
        });
        if lines.lines.len() > KEPT {
            lines.lines.pop_front();
        }

        self.changed(lines);
    }
}

impl Logs {
    fn lock(&self) -> MutexGuard<'_, Lines> {
        self.0.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn changed(&self, mut lines: MutexGuard<'_, Lines>) {
        lines.revision += 1;
        drop(lines);
        self.0.1.notify_all();
    }

    pub fn clear(&self) {
        let mut lines = self.lock();
        lines.lines.clear();
        self.changed(lines);
    }

    fn revision(&self) -> u64 {
        self.lock().revision
    }

    fn recent(&self, max_level: log::Level) -> Vec<LogLine> {
        self.lock()
            .lines
            .iter()
            .rev()
            .filter(|line| line.level <= max_level)
            .take(SHOWN)
            .cloned()
            .collect()
    }

    /// Blocks until the revision moves past `seen`, or `cancel` gets cancelled.
    fn wait_for_change(&self, seen: u64, cancel: &CancellationToken) -> u64 {
        let mut lines = self.lock();
        while lines.revision == seen && !cancel.is_cancelled() {
            lines = self
                .0
                .1
                .wait_timeout(lines, CANCEL_POLL)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        lines.revision
    }
}

//
// MARK: window
//

#[derive(Clone, PartialEq)]
pub struct LogWindowInput {
    pub process: String,
    pub logs: Logs,
}

pub struct LogWindow {
    logs: Logs,
    max_level: log::Level,
}

pub enum LogMessage {
    Changed(u64),
    MaxLevel(Option<usize>),
    Clear,
}

/// Waits on a background thread for the book to change past `seen`.
fn watch(book: &Logs, seen: u64, context: &ComponentContext<LogWindow>) {
    let book = book.clone();
    _ = context
        .spawn_background(move |cancel| LogMessage::Changed(book.wait_for_change(seen, &cancel)));
}

impl Component for LogWindow {
    type Input = LogWindowInput;
    type Message = LogMessage;

    fn create(input: &LogWindowInput, context: &ComponentContext<Self>) -> Self {
        watch(&input.logs, input.logs.revision(), context);
        Self {
            logs: input.logs.clone(),
            max_level: log::Level::Trace,
        }
    }

    fn update(&mut self, message: LogMessage, context: &ComponentContext<Self>) {
        match message {
            // the window redraws after every update anyway, so we just need to start waiting
            // again
            LogMessage::Changed(revision) => watch(&self.logs, revision, context),
            LogMessage::MaxLevel(index) => {
                if let Some(level) = index.and_then(|index| ALL_LEVELS.get(index)) {
                    self.max_level = *level;
                }
            }
            LogMessage::Clear => self.logs.clear(),
        }
    }

    fn view(&self, input: &LogWindowInput, context: &mut ViewContext<Self>) -> View {
        let title = format!("{} logs", input.process);
        context.window_title(title.clone());
        context.window_visuals(
            WindowVisuals::new()
                .backdrop(WindowBackdrop::Mica)
                .client_size(820.0, 520.0)
                .constraints(WindowConstraints {
                    min_width: Some(360.0),
                    min_height: Some(240.0),
                    ..Default::default()
                }),
        );

        let lines = input.logs.recent(self.max_level);
        let body: View = if lines.is_empty() {
            TextBlock::new()
                .text("No logs yet")
                .opacity(0.7)
                .margin(Thickness::uniform(16.0))
                .into()
        } else {
            // everything goes in one block so you can select across lines. runs can't have
            // their own color though, so errors are bold and warnings are italic instead
            let paragraphs = lines.into_iter().map(|line| {
                RichTextParagraph::new([RichTextInline::Run(RichTextRun {
                    text: line.text,
                    is_bold: line.level == log::Level::Error,
                    is_italic: line.level == log::Level::Warn,
                })])
            });

            ScrollViewer::new().content(
                Border::new().padding(Thickness::uniform(16.0)).content(
                    RichTextBlock::new()
                        .paragraphs(RichText::new(paragraphs))
                        .text_wrapping(TextWrapping::Wrap)
                        .is_text_selection_enabled(true),
                ),
            )
        };

        let selected = ALL_LEVELS.iter().position(|level| *level == self.max_level);
        let toolbar = Grid::new()
            .columns([GridLength::STAR, GridLength::Auto])
            .margin(Thickness::new(16.0, 12.0, 16.0, 0.0))
            .children((
                ComboBox::new()
                    .items_source(ALL_LEVELS.map(label))
                    .selected_index(selected)
                    .on_selection_changed(context.callback(LogMessage::MaxLevel))
                    .min_width(160.0),
                Button::new()
                    .on_click(context.callback(|()| LogMessage::Clear))
                    .grid_column(1)
                    .content(icon_label(Symbol::Clear, "Clear")),
            ));

        let background = Border::new()
            .grid_row(1)
            .background(ThemeBrush::CardBackground)
            .border_brush(ThemeBrush::CardStroke)
            .border_thickness(Thickness::new(0.0, 1.0, 0.0, 0.0))
            .content(
                Grid::new()
                    .rows([GridLength::Auto, GridLength::STAR])
                    .children((toolbar, Border::new().grid_row(1).content(body))),
            );

        Grid::new()
            .rows([GridLength::Auto, GridLength::STAR])
            .children((TitleBar::new().title(title), background))
    }
}
