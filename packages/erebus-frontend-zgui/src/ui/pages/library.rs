use erebus_frontend::i18n;
use zgui::prelude::*;
use zgui::reactive::{RenderEffect, StoredValue};
use zgui::reactive::{RwSignal, UnsyncCallback};
use zgui::vocab::{Key, NamedKey};
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;
use zgui_ui_primitives::{PresenceProps, use_presence};

use super::PageStyle;

use crate::{
    components::{
        fab::FloatingActionButtonProps,
        octicons::{ARROW_DOWN, ARROW_UP, PAINTBRUSH, PLUS, SEARCH, TRASH},
        tooltip::StyledTooltipProps,
    },
    state::AppState,
    ui::shell::{HeaderProps, LibraryMenuItemsProps, MainMenuProps},
};

struct LibraryTheme {
    id: String,
    title: String,
    subtitle: String,
    description: String,
    previous: Option<String>,
    next: Option<String>,
}

#[component]
pub fn LibraryPage() -> impl IntoView {
    let app = AppState::get();
    let count = Signal::derive_local(move || {
        i18n::theme_count(
            app.library
                .with(|library| library.config_store.themes.len()),
        )
    });
    let searching = RwSignal::new_local(!app.library_search.get_untracked().is_empty());
    // closing it clears the search, so the whole library is back
    let set_searching = move |on: bool| {
        searching.set(on);
        if !on {
            app.library_search.set(String::new());
        }
    };
    let search = Binding::controlled(Signal::derive_local(move || searching.get()), set_searching);

    install_stylesheet(PageStyle::CLASS, PageStyle::CSS);
    let search_bar = NodeRef::new();

    view! {
        column(
            class = PageStyle::CLASS,
            class:fab = move || app.narrow.get(),
            on:drop = move |event| app.install_themes(&event.paths)) {
            Header(title = "Theme Library".to_owned(), subtitle = count) {
                StyledTooltip(label = "Search") {
                    Toggle(pressed = search, size = ToggleSize::Sm, label = "Search") {
                        Icon(icon = SEARCH, label = "")
                    }
                }
                // narrow, it floats over the page instead
                if move || !app.narrow.get() {
                    Button(size = ButtonSize::Sm, on:click = move |_| app.pick_theme()) {
                        Icon(icon = PLUS, label = "")
                        "Add Theme"
                    }
                } else {}
                MainMenu {LibraryMenuItems()}
            }
            Presence(present = Signal::derive_local(move || searching.get()), surface = search_bar) {
                SearchBar(element_ref = search_bar, close = UnsyncCallback::new(move |()| set_searching(false)))
            }
            scroll(class = "scroll") {
                column(class = "body") {
                    {move || library_content(app)}
                }
            }
            if move || app.narrow.get() {
                row(class = "add-button") {
                    FloatingActionButton(label = "Add Theme", on_click = UnsyncCallback::new(move |()| app.pick_theme()))
                }
            } else {}
        }
    }
}

/// Keep the search field mounted through its closing animation with
/// [`Presence`](zgui_ui_primitives::Presence). Removing it early would make the list jump.
#[component]
fn SearchBar(element_ref: NodeRef, close: UnsyncCallback<()>) -> impl IntoView {
    let app = AppState::get();
    let presence = use_presence();
    // Escape should clear the search too, just like closing it with the button.
    let on_key_down = move |ev: &mut EventCx<'_, zgui::view::events::KeyDown>| {
        if ev.key == Key::Named(NamedKey::Escape) {
            ev.stop_propagation();
            close.run(());
        }
    };

    // Focus the field when it opens. Its handle isn't bound on the first run; reading it here
    // makes the effect run again once the field has been built.
    let field = NodeRef::new();
    let focused = StoredValue::new_local(false);
    let focus = RenderEffect::new(move |_| {
        if field.get().is_some() && !focused.get_value() {
            focused.set_value(true);
            field.focus();
        }
    });
    on_cleanup_local(move || drop(focus));

    view! {
        row(
            class = "search",
            node_ref = element_ref,
            on:key_down = on_key_down,
            attr:data-state = move || presence.map(|presence| presence.state_name().to_owned())
        ) {
            InputGroup {
                InputGroupAddon {Icon(icon = SEARCH, label = "")}
                InputGroupInput(
                    node_ref = field,
                    value = app.library_search,
                    on_change = UnsyncCallback::new(move |text: String| app.library_search.set(text)),
                    placeholder = "Search themes",
                    a11y:label = "Search themes"
                )
            }
        }
    }
}

fn library_content(app: AppState) -> AnyView {
    let query = app.library_search.get();
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    // positions in a filtered list are ambiguous, so only reorder unfiltered
    let reorderable = terms.is_empty();

    let (total, themes) = app.library.with(|library| {
        let entries = &library.config_store.themes;
        let themes: Vec<LibraryTheme> = entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let info = library.theme_info(entry);
                let haystack = info.search_text();
                terms
                    .iter()
                    .all(|term| haystack.contains(term))
                    .then(|| LibraryTheme {
                        id: entry.id.clone(),
                        title: info.title.clone(),
                        subtitle: info.subtitle.clone(),
                        description: info.description.to_owned(),
                        previous: index
                            .checked_sub(1)
                            .and_then(|index| entries.get(index))
                            .map(|entry| entry.id.clone()),
                        next: entries.get(index + 1).map(|entry| entry.id.clone()),
                    })
            })
            .collect();
        (entries.len(), themes)
    });

    if total == 0 {
        return AnyView::new(view! {
            Empty {
                EmptyHeader {
                    EmptyMedia(variant = EmptyMediaVariant::Icon) {Icon(icon = PAINTBRUSH, label = "")}
                    EmptyTitle {"Your Library Is Empty"}
                    EmptyDescription {"Add a theme file (.wasm), or drag-and-drop one anywhere on this page to get started."}
                }
            }
        });
    }

    let rows: Vec<_> = themes
        .into_iter()
        .map(|theme| library_row(app, theme, reorderable).into_view())
        .collect();
    // Keep the empty state outside the group so it can center in the full page.
    if rows.is_empty() {
        return AnyView::new(view! {
            Empty {
                EmptyHeader {
                    EmptyMedia(variant = EmptyMediaVariant::Icon) {Icon(icon = SEARCH, label = "")}
                    EmptyTitle {"No Results Found"}
                    EmptyDescription {"Try a different search"}
                }
            }
        });
    }
    let results = AnyView::new(view! { column(class = "list") {{rows}} });

    AnyView::new(view! {
        SettingsGroup {
            {results}
        }
    })
}

fn library_row(app: AppState, theme: LibraryTheme, reorderable: bool) -> impl IntoView {
    let LibraryTheme {
        id,
        title,
        subtitle,
        description,
        previous,
        next,
    } = theme;

    let mover = |target: Option<String>, label: &'static str, icon| {
        let id = id.clone();
        let disabled = target.is_none();
        view! {
            StyledTooltip(label = label) {
                Button(
                    variant = ButtonVariant::Ghost,
                    size = ButtonSize::IconSm,
                    disabled = disabled,
                    a11y:label = label,
                    on:click = move |_| {
                        if let Some(target) = &target {
                            app.move_theme(&id, target);
                        }
                    }
                ) {
                    Icon(icon = icon, label = "")
                }
            }
        }
    };
    let reorder = reorderable.then(|| {
        (
            mover(previous, "Move Up", ARROW_UP).into_view(),
            mover(next, "Move Down", ARROW_DOWN).into_view(),
        )
    });

    let remove = {
        let title = title.clone();
        move |_: &mut EventCx<'_, zgui::view::events::Click>| app.confirm_remove_theme(&id, &title)
    };

    view! {
        Item(variant = ItemVariant::Outline, class = "card") {
            ItemContent {
                ItemTitle {{title}}
                ItemDescription {{subtitle}}
                {(!description.is_empty()).then(|| view! { ItemDescription {{description}} }.into_view())}
            }
            ItemActions {
                {reorder}
                StyledTooltip(label = "Remove from Library") {
                    Button(
                        variant = ButtonVariant::Destructive,
                        size = ButtonSize::IconSm,
                        a11y:label = "Remove from Library",
                        on:click = remove
                    ) {
                        Icon(icon = TRASH, label = "")
                    }
                }
            }
        }
    }
}
