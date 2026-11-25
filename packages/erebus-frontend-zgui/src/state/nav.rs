use std::time::Duration;

use zgui::prelude::*;
use zgui::reactive::{LocalStorage, RenderEffect, StoredValue};
use zgui::view::{TimeoutHandle, set_timeout};

use super::AppState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Library,
    App(String),
}

/// Below this frame width, the sidebar and the page are shown one at a time.
const BREAKPOINT: f32 = 640.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nav {
    /// from the sidebar to a page
    Push,
    /// back to the sidebar
    Pop,
}

pub(super) fn narrow_window() -> Signal<bool, LocalStorage> {
    let window = use_window();
    Signal::derive_local(move || window.size().get().width.0 < BREAKPOINT)
}

impl AppState {
    /// Animate when navigating between panes in a narrow window. Resizing across the breakpoint
    /// also changes which panes are visible, but shouldn't trigger a navigation animation.
    pub(super) fn follow_navigation(self) {
        let clear = StoredValue::new_local(None::<TimeoutHandle>);
        let effect = RenderEffect::new(move |previous: Option<bool>| {
            let showing = self.showing_page.get();
            if previous.is_some_and(|previous| previous != showing) && self.narrow.get_untracked() {
                self.nav
                    .set(Some(if showing { Nav::Push } else { Nav::Pop }));
                // a little longer than the animation; it only has to be over by the next resize
                clear.set_value(Some(set_timeout(Duration::from_millis(600), move || {
                    self.nav.set(None)
                })));
            }
            showing
        });
        StoredValue::new_local(effect);
    }

    /// In a narrow window, selecting a page also hides the sidebar.
    pub fn open(self, selection: Selection) {
        self.selection.set(selection);
        self.showing_page.set(true);
    }
}
