//! Items in the navigation pane.

use std::collections::BTreeSet;

use erebus_frontend::i18n;
use windows_reactor::*;

use super::{Erebus, Message, Selection, widgets};

fn two_line_content_nowrap(title: &str, subtitle: &str) -> View {
    StackPanel::new()
        .spacing(2.0)
        .margin(Thickness::new(0.0, 8.0, 0.0, 8.0))
        .children((
            TextBlock::new()
                .text(title)
                .text_trimming(TextTrimming::CharacterEllipsis),
            TextBlock::new()
                .text(subtitle)
                .text_trimming(TextTrimming::CharacterEllipsis)
                .font_size(12.0)
                .opacity(0.7),
        ))
}

fn item(selection: &Selection, selected: bool, content: View, icon: View) -> KeyedView {
    let tag = selection.tag();
    let slots = [
        SlotView::new(NavigationViewItemSlot::Content, content),
        SlotView::new(NavigationViewItemSlot::Icon, icon),
    ];

    KeyedView::new(
        tag.clone(),
        NavigationViewItem::new()
            .tag(tag)
            .is_selected(selected)
            .selects_on_invoked(true)
            .slots(slots),
    )
}

impl Erebus {
    pub(super) fn sidebar_items(&self) -> Vec<KeyedView> {
        let themes = self.library.config_store.themes.len();
        let badge: View = if themes == 0 {
            View::empty()
        } else {
            // NavigationViewItem keeps a bunch of padding on the right for a chevron, even
            // when there isn't one, so the badge reaches into it to line up with the others
            InfoBadge::new()
                .margin(Thickness::new(0.0, 0.0, -12.0, 0.0))
                .value(themes as i32)
                .into()
        };
        let library = item(
            &Selection::Library,
            self.selection == Selection::Library,
            widgets::row(TextBlock::new().text("Theme Library"), badge),
            SymbolIcon::new().symbol(Symbol::Library).into(),
        );

        let known_app_names: BTreeSet<&String> = self
            .library
            .config_store
            .apps
            .keys()
            .chain(self.runtime.running())
            .collect();

        let mut apps: Vec<KeyedView> = known_app_names
            .into_iter()
            .map(|process| {
                let instances = self.runtime.instances(process).len();
                let selection = Selection::App(process.clone());
                let selected = self.selection == selection;

                item(
                    &selection,
                    selected,
                    widgets::row(
                        two_line_content_nowrap(process, &i18n::status(instances)),
                        widgets::status_dot(instances > 0),
                    ),
                    // generic app icon from Segoe Fluent
                    FontIcon::new().glyph("\u{ECAA}").into(),
                )
            })
            .collect();

        if apps.is_empty() {
            apps.push(KeyedView::new(
                "no-apps",
                NavigationViewItem::new().selects_on_invoked(false).slot(
                    NavigationViewItemSlot::Content,
                    Border::new()
                        .opacity(0.7)
                        .content(two_line_content_nowrap("No apps yet", "Start one with erebus to see it here")),
                ),
            ));
        }

        let apps = KeyedView::new(
            "apps",
            NavigationViewItem::new()
                .tag("apps")
                .selects_on_invoked(false)
                .is_expanded(true)
                .slots([
                    SlotView::new(NavigationViewItemSlot::Content, "Apps"),
                    SlotView::new(
                        NavigationViewItemSlot::Icon,
                        SymbolIcon::new().symbol(Symbol::AllApps),
                    ),
                    SlotView::collection(NavigationViewItemSlot::MenuItems, apps),
                ]),
        );

        vec![library, apps]
    }
}

/// NavigationView only lays out footer items it made itself, so this is a lookalike that
/// opens the about dialog instead by wrapping a dummy item in a `Border` with an event handler.
pub(super) fn about_item(context: &mut ViewContext<Erebus>) -> View {
    Border::new()
        .on_pointer_released(context.callback(|_| Message::About(true)))
        .content(NavigationViewItem::new().selects_on_invoked(false).slots([
            SlotView::new(NavigationViewItemSlot::Content, "About erebus"),
            SlotView::new(
                NavigationViewItemSlot::Icon,
                FontIcon::new().glyph("\u{E946}"),
            ),
        ]))
}
