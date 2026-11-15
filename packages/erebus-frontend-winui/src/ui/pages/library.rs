//! Theme library page

use erebus_frontend::i18n;
use windows_reactor::*;

use crate::ui::{
    Erebus, Message,
    actions::{MOVE_DOWN, MOVE_UP, OPEN_CONFIG_FOLDER, RELOAD_LIBRARY, REMOVE},
    widgets::{
        card, empty_state, icon_label, menu_button, menu_item, page, row, section, subtitle,
    },
};

impl Erebus {
    pub(crate) fn library_page(&self, context: &mut ViewContext<Self>) -> View {
        let actions = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(8.0)
            .children((
                Button::new()
                    .style(ButtonStyle::Accent)
                    .on_click(context.callback(|()| Message::PickThemes))
                    .content(icon_label(Symbol::Add, "Add Theme")),
                menu_button(
                    vec![
                        menu_item(OPEN_CONFIG_FOLDER, true),
                        menu_item(RELOAD_LIBRARY, true),
                    ],
                    context.callback(Message::LibraryMenu),
                ),
            ));

        let themes = &self.library.config_store.themes;
        let (sections, fill) = if themes.is_empty() {
            let empty = empty_state(
                FontIcon::new().glyph("\u{E790}"),
                "Your library is empty",
                "Add a theme file (.wasm), or drop one anywhere on this page.",
                Button::new()
                    .on_click(context.callback(|()| Message::PickThemes))
                    .content("Browse Files"),
            );
            (Vec::new(), Some(empty))
        } else {
            let rows = self.theme_rows(context);
            let no_results = rows.is_empty().then(|| {
                empty_state(
                    SymbolIcon::new().symbol(Symbol::Find),
                    &format!("No results for \"{}\"", self.search.trim()),
                    "Try a different search, or clear it to see every theme.",
                    Button::new()
                        .on_click(context.callback(|()| Message::Search(String::new())))
                        .content("Clear Search"),
                )
            });

            let section = section(
                "Themes",
                "Add or drop theme files (.wasm) here. Use the menu to change the order they're applied in.",
                [
                    KeyedView::new(
                        "search",
                        TextBox::new()
                            .text(self.search.clone())
                            .placeholder_text("Search themes")
                            .on_text_changed(context.callback(Message::Search))
                            .margin(Thickness::new(0.0, 0.0, 0.0, 8.0)),
                    ),
                    KeyedView::new("list", StackPanel::new().spacing(6.0).keyed_children(rows)),
                ],
            );
            (vec![KeyedView::new("themes", section)], no_results)
        };

        let highlight = if self.drop_hover {
            Brush::from(ThemeBrush::Accent)
        } else {
            Brush::from(Color::argb(0, 0, 0, 0))
        };

        Border::new()
            .border_brush(highlight)
            .border_thickness(2.0)
            .background(Color::argb(0, 0, 0, 0))
            .drop_policy(DragDropPolicy::new().storage_items(
                DragDropAction::new(DragDropOperation::Copy).caption("Add to library"),
            ))
            .on_drag_enter(context.callback(|_| Message::DropHover(true)))
            .on_drag_over(context.callback(|_| Message::DropHover(true)))
            .on_drag_leave(context.callback(|()| Message::DropHover(false)))
            .on_drop(context.callback(Message::Dropped))
            .content(page(
                self.page_width(),
                "Theme Library",
                &i18n::theme_count(themes.len()),
                actions,
                sections,
                fill,
            ))
    }

    /// Cards for every theme that matches the search.
    fn theme_rows(&self, context: &mut ViewContext<Self>) -> Vec<KeyedView> {
        let themes = &self.library.config_store.themes;
        // moving things around in a filtered list is confusing, so it only works unfiltered
        let reorderable = self.search.trim().is_empty();

        themes
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let info = self.library.theme_info(entry);

                let has_results = self
                    .search
                    .split_whitespace()
                    .all(|term| info.search_text().contains(&term.to_lowercase()));

                if !has_results {
                    return None;
                }

                let id = entry.id.clone();
                let menu = menu_button(
                    vec![
                        menu_item(MOVE_UP, reorderable && index > 0),
                        menu_item(MOVE_DOWN, reorderable && index + 1 < themes.len()),
                        MenuItem::separator("separator"),
                        menu_item(REMOVE, true),
                    ],
                    context.callback(move |item| Message::ThemeMenu {
                        id: id.clone(),
                        item,
                    }),
                );

                let mut text = vec![
                    KeyedView::new(
                        "title",
                        TextBlock::new()
                            .text(info.title.clone())
                            .font_weight(FontWeight::SEMI_BOLD),
                    ),
                    KeyedView::new("subtitle", subtitle(info.subtitle.clone()).font_size(12.0)),
                ];
                if !info.description.is_empty() {
                    text.push(KeyedView::new(
                        "description",
                        TextBlock::new()
                            .text(info.description)
                            .text_wrapping(TextWrapping::Wrap)
                            .margin(Thickness::new(0.0, 4.0, 0.0, 0.0)),
                    ));
                }

                let row = row(
                    StackPanel::new()
                        .keyed_children(text)
                        .tooltip(format!("Installed from {}", entry.source.display())),
                    menu,
                );
                Some(KeyedView::new(entry.id.as_str(), card(row)))
            })
            .collect()
    }

    pub(crate) fn remove_dialog(&self, context: &mut ViewContext<Self>) -> View {
        let name = self.removing.as_ref().map_or("", |(_, name)| name);

        ContentDialog::new()
            .title("Remove theme?")
            .primary_button_text("Remove")
            .close_button_text("Cancel")
            .is_open(self.removing.is_some())
            .on_closed(context.callback(Message::RemoveClosed))
            .content(TextBlock::new().text_wrapping(TextWrapping::Wrap).text(format!(
                "\"{name}\" will be removed from the library, along with its settings in every app."
            )))
    }
}
