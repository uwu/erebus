//! Application page

use chrono::{DateTime, Local};
use erebus_frontend::{apps::BackendInstance, i18n};
use erebus_ipc::config::ThemeEntry;
use windows_reactor::*;

use super::settings::setting_row;
use crate::ui::{
    Erebus, Message, Selection,
    actions::{FORGET_APP, VIEW_LOGS},
    widgets::{
        PAGE_PADDING, bare_switch, card, empty_state, menu_button, menu_item, page, row, section,
        title_and_subtitle,
    },
};

impl Erebus {
    pub(crate) fn app_page(&self, process: &str, context: &mut ViewContext<Self>) -> View {
        let instances = self.runtime.instances(process);

        let name = process.to_owned();
        let menu = menu_button(
            vec![
                menu_item(VIEW_LOGS, true),
                menu_item(FORGET_APP, instances.is_empty()),
            ],
            context.callback(move |item| Message::AppMenu {
                process: name.clone(),
                item,
            }),
        );

        let mut sections = Vec::new();
        if !instances.is_empty() {
            sections.push(KeyedView::new("processes", processes_section(instances)));
        }

        let themes = &self.library.config_store.themes;
        let fill = if themes.is_empty() {
            Some(empty_state(
                FontIcon::new().glyph("\u{E790}"),
                "No themes yet",
                "Themes in your library can be turned on and set up for each app here.",
                Button::new()
                    .on_click(
                        context.callback(|()| Message::Navigate(Some(Selection::Library.tag()))),
                    )
                    .content("Go to Theme Library"),
            ))
        } else {
            let rows = themes.iter().map(|entry| {
                KeyedView::new(
                    entry.id.as_str(),
                    self.app_theme_row(process, entry, context),
                )
            });
            sections.push(KeyedView::new(
                "themes",
                section(
                    "Themes",
                    "Themes are applied in the order of your library.",
                    rows.collect::<Vec<_>>(),
                ),
            ));
            None
        };

        page(
            self.page_width(),
            process,
            &i18n::status(instances.len()),
            menu,
            sections,
            fill,
        )
    }

    fn app_theme_row(
        &self,
        process: &str,
        entry: &ThemeEntry,
        context: &mut ViewContext<Self>,
    ) -> View {
        let info = self.library.theme_info(entry);
        let theme = self.library.app_theme(process, &entry.id);

        let (name, id) = (process.to_owned(), entry.id.clone());
        let switch = bare_switch(
            theme.is_some_and(|theme| theme.enabled),
            context.callback(move |enabled| Message::SetEnabled {
                process: name.clone(),
                id: id.clone(),
                enabled,
            }),
        );

        let header = Border::new()
            .margin(Thickness::new(0., 8., 0., 8.))
            .content(row(title_and_subtitle(&info.title, &info.subtitle), switch));
        let header = if info.description.is_empty() {
            header
        } else {
            header.tooltip(info.description)
        };

        if info.settings.is_empty() {
            return card(header);
        }

        let settings = info.settings.iter().map(|field| {
            let (name, id, key) = (process.to_owned(), entry.id.clone(), field.key.clone());
            let set = move |value| Message::SetSetting {
                process: name.clone(),
                id: id.clone(),
                key: key.clone(),
                value,
            };
            let current = theme.and_then(|theme| theme.settings.get(&field.key));
            KeyedView::new(
                field.key.as_str(),
                setting_row(field, current, set, context),
            )
        });

        // expander centers content inside it, so settings fields have to get its complete
        // width up, so calclate that here
        let width = self.page_width() - 2.0 * PAGE_PADDING - 2.0 - 32.0;

        let key = format!("{process}/{}", entry.id);
        Expander::new()
            .is_expanded(self.expanded.contains(&key))
            .on_is_expanded_changed(context.callback(move |expanded| Message::Expanded {
                key: key.clone(),
                expanded,
            }))
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .slots([
                SlotView::new(ExpanderSlot::Header, header),
                SlotView::new(
                    ExpanderSlot::Content,
                    StackPanel::new()
                        .width(width.max(0.0))
                        .spacing(12.0)
                        .keyed_children(settings.collect::<Vec<_>>()),
                ),
            ])
    }
}

fn processes_section(instances: &[BackendInstance]) -> View {
    let rows = instances.iter().map(|instance| {
        let connected_at = DateTime::<Local>::from(instance.connected_at);
        let subtitle = format!(
            "{} (protocol v{}) connected at {}",
            instance.backend,
            instance.version,
            connected_at.format("%H:%M:%S"),
        );

        KeyedView::new(
            u64::from(instance.pid),
            card(
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(12.0)
                    .children((
                        SymbolIcon::new()
                            .symbol(Symbol::Page)
                            .vertical_alignment(VerticalAlignment::Center),
                        title_and_subtitle(&format!("PID {}", instance.pid), &subtitle),
                    )),
            ),
        )
    });

    section("Processes", "", rows.collect::<Vec<_>>())
}
