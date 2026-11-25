use std::{collections::HashMap, rc::Rc};

use chrono::{DateTime, Local};
use erebus_frontend::i18n;
use erebus_theme::settings::{OwnedSettingsField, SettingValue};
use zgui::prelude::*;
use zgui::reactive::RwSignal;
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;

use super::PageStyle;

use crate::{
    components::{
        octicons::{CHEVRON_DOWN, PAINTBRUSH},
        tooltip::StyledTooltipProps,
    },
    state::AppState,
    ui::{
        shell::{AppMenuItemsProps, HeaderProps, MainMenuProps},
        theme_settings::SettingRowProps,
    },
};

#[derive(Clone)]
struct AppTheme {
    id: String,
    title: String,
    subtitle: String,
    description: String,
    settings: Rc<Vec<OwnedSettingsField>>,
    enabled: bool,
    values: Rc<HashMap<String, SettingValue>>,
}

#[component]
pub fn AppPage(app_id: String) -> impl IntoView {
    let app = AppState::get();

    let title = {
        let app_id = app_id.clone();
        Signal::derive_local(move || {
            app.library
                .with(|library| library.app_name(&app_id).to_owned())
        })
    };
    let status = {
        let app_id = app_id.clone();
        Signal::derive_local(move || i18n::status(app.instances(&app_id)))
    };

    let processes = {
        let app_id = app_id.clone();
        move || {
            let rows: Vec<_> = app.runtime.with(|runtime| {
                runtime
                    .instances(&app_id)
                    .iter()
                    .map(|instance| {
                        let time: DateTime<Local> = instance.connected_at.into();
                        let pid = format!("PID {}", instance.pid);
                        let backend = instance.backend.clone();
                        let meta = format!(
                            "protocol v{} connected at {}",
                            instance.version,
                            time.format("%H:%M:%S")
                        );
                        view! {
                            Item(variant = ItemVariant::Outline, class = "card") {
                                ItemContent {
                                    ItemTitle {
                                        text {{pid}}
                                        Badge(variant = BadgeVariant::Secondary) {{backend}}
                                    }
                                    ItemDescription {{meta}}
                                }
                            }
                        }
                        .into_view()
                    })
                    .collect()
            });

            (!rows.is_empty()).then(|| {
                view! {
                    SettingsGroup {
                        SettingsGroupLabel {"Processes"}
                        column(class = "list") {{rows}}
                    }
                }
                .into_view()
            })
        }
    };

    let no_themes = move || {
        app.library
            .with(|library| library.config_store.themes.is_empty())
    };

    install_stylesheet(PageStyle::CLASS, PageStyle::CSS);
    let menu_app_id = app_id.clone();

    view! {
        column(class = PageStyle::CLASS) {
            Header(title = title, subtitle = status) {
                MainMenu {AppMenuItems(app_id = menu_app_id.clone())}
            }
            scroll(class = "scroll") {
                column(class = "body") {
                    {processes}
                    if move || no_themes() {
                        Empty {
                            EmptyHeader {
                                EmptyMedia(variant = EmptyMediaVariant::Icon) {Icon(icon = PAINTBRUSH, label = "")}
                                EmptyTitle {"No Themes Installed"}
                                EmptyDescription {"Themes you add to your library will show up here."}
                            }
                        }
                    } else {
                        AppThemes(app_id = app_id.clone())
                    }
                }
            }
        }
    }
}

#[component]
fn AppThemes(app_id: String) -> impl IntoView {
    let app = AppState::get();
    let themes = {
        let app_id = app_id.clone();
        move || {
            let themes: Vec<AppTheme> = app.library.with(|library| {
                library
                    .config_store
                    .themes
                    .iter()
                    .map(|entry| {
                        let info = library.theme_info(entry);
                        let (enabled, values) = library
                            .app_theme(&app_id, &entry.id)
                            .map(|theme| (theme.enabled, theme.settings.clone()))
                            .unwrap_or_default();
                        AppTheme {
                            id: entry.id.clone(),
                            title: info.title.clone(),
                            subtitle: info.subtitle.clone(),
                            description: info.description.to_owned(),
                            settings: Rc::new(info.settings.to_vec()),
                            enabled,
                            values: Rc::new(values),
                        }
                    })
                    .collect()
            });

            themes
                .into_iter()
                .map(|theme| {
                    view! { AppThemeRow(app_id = app_id.clone(), theme = theme) }.into_view()
                })
                .collect::<Vec<_>>()
        }
    };

    view! {
        SettingsGroup {
            SettingsGroupLabel {"Themes"}
            column(class = "list") {{themes}}
        }
    }
}

#[component]
fn AppThemeRow(app_id: String, theme: AppTheme) -> impl IntoView {
    let app = AppState::get();
    let AppTheme {
        id,
        title,
        subtitle,
        description,
        settings,
        enabled,
        values,
    } = theme;

    let key = format!("{app_id}/{id}");
    let open = RwSignal::new_local(app.is_expanded(&key));
    let expanded = Binding::controlled(Signal::derive_local(move || open.get()), move |now| {
        open.set(now);
        app.set_expanded(&key, now);
    });

    let enabled = RwSignal::new_local(enabled);
    let checked = {
        let (app_id, id) = (app_id.clone(), id.clone());
        Binding::controlled(
            Signal::derive_local(move || enabled.get()),
            move |on: bool| {
                enabled.set(on);
                app.set_enabled(&app_id, &id, on);
            },
        )
    };

    let rows: Vec<_> = settings
        .iter()
        .map(|field| {
            let set = {
                let (app_id, id, key) = (app_id.clone(), id.clone(), field.key.clone());
                Rc::new(move |value| app.set_setting(&app_id, &id, &key, value))
            };
            view! {
                SettingRow(
                    field = field.clone(),
                    current = values.get(&field.key).cloned(),
                    on_change = set
                )
            }
            .into_view()
        })
        .collect();
    let has_settings = !rows.is_empty();
    let label = title.clone();

    // Build both children inside the collapsible so they can find its context.
    view! {
        Collapsible(open = expanded, class = "card") {
            Item(class = "heading") {
                ItemContent {
                    ItemTitle {{title}}
                    ItemDescription {{subtitle}}
                    {(!description.is_empty()).then(|| view! { ItemDescription {{description}} }.into_view())}
                }
                ItemActions {
                    Switch(checked = checked, a11y:label = label)
                    row {
                        {has_settings.then(|| view! {
                            StyledTooltip(label = "Theme Settings") {
                                CollapsibleTrigger(class = "expander", a11y:label = "Theme Settings") {
                                    Icon(
                                        icon = CHEVRON_DOWN,
                                        label = "",
                                        class = "chevron",
                                        class:open = move || open.get()
                                    )
                                }
                            }
                        }.into_view())}
                    }
                }
            }
            {has_settings.then(|| view! {
                CollapsibleContent {
                    column(class = "settings") {{rows}}
                }
            }.into_view())}
        }
    }
}
