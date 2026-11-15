//! common UI patterns that get used throughout over the app.

use windows_reactor::*;

use super::TITLE_BAR_HEIGHT;

const REPO: &str = "https://github.com/uwu/erebus";

/// how much space goes around a page's content
pub const PAGE_PADDING: f64 = 48.0;

pub fn subtitle(text: impl Into<String>) -> TextBlock {
    TextBlock::new()
        .text(text)
        .text_wrapping(TextWrapping::Wrap)
        .opacity(0.7)
}

pub fn page(
    width: f64,
    title: &str,
    subtitle_text: &str,
    actions: impl Into<View>,
    sections: impl IntoIterator<Item = KeyedView>,
    fill: Option<View>,
) -> View {
    let header = Grid::new()
        .columns([GridLength::STAR, GridLength::Auto])
        .children((
            StackPanel::new().spacing(2.0).children((
                TextBlock::new()
                    .text(title)
                    .font_size(28.0)
                    .font_weight(FontWeight::SEMI_BOLD),
                subtitle(subtitle_text),
            )),
            Border::new()
                .grid_column(1)
                .vertical_alignment(VerticalAlignment::Center)
                .content(actions),
        ));

    let children = std::iter::once(KeyedView::new("header", header)).chain(sections);
    ScrollViewer::new().content(
        Border::new()
            .width(width)
            .horizontal_alignment(HorizontalAlignment::Center)
            .padding(Thickness::new(
                PAGE_PADDING,
                PAGE_PADDING - TITLE_BAR_HEIGHT,
                PAGE_PADDING,
                PAGE_PADDING,
            ))
            .content(
                Grid::new()
                    .rows([GridLength::Auto, GridLength::STAR])
                    .children((
                        StackPanel::new().spacing(24.0).keyed_children(children),
                        Border::new()
                            .grid_row(1)
                            .content(fill.unwrap_or_else(View::empty)),
                    )),
            ),
    )
}

/// A group of rows with a title on top.
pub fn section(title: &str, description: &str, rows: impl IntoIterator<Item = KeyedView>) -> View {
    let mut header = vec![KeyedView::new(
        "title",
        TextBlock::new()
            .text(title)
            .font_size(16.0)
            .font_weight(FontWeight::SEMI_BOLD),
    )];
    if !description.is_empty() {
        header.push(KeyedView::new("description", subtitle(description)));
    }
    let header = StackPanel::new()
        .spacing(2.0)
        .margin(Thickness::new(0., 0., 0., 12.))
        .keyed_children(header);

    let children = std::iter::once(KeyedView::new("header", header)).chain(rows);
    StackPanel::new().spacing(6.0).keyed_children(children)
}

pub fn card(content: impl Into<View>) -> View {
    Border::new()
        .background(ThemeBrush::CardBackground)
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(1.0)
        .corner_radius(8.0)
        .padding(Thickness::new(16.0, 12.0, 16.0, 12.0))
        .content(content)
}

/// Fills in for stuff that doesn't exist.
pub fn empty_state(
    icon: impl Into<View>,
    title: &str,
    description: &str,
    action: impl Into<View>,
) -> View {
    StackPanel::new()
        .spacing(4.0)
        .max_width(480.)
        .margin(Thickness::new(0., 32., 0., 32.))
        .horizontal_alignment(HorizontalAlignment::Center)
        .vertical_alignment(VerticalAlignment::Center)
        .children((
            Viewbox::new()
                .width(48.0)
                .height(48.0)
                .opacity(0.7)
                .horizontal_alignment(HorizontalAlignment::Center)
                .margin(Thickness::new(0.0, 0.0, 0.0, 12.0))
                .slot(ViewboxSlot::Child, icon),
            TextBlock::new()
                .text(title)
                .font_size(20.0)
                .text_wrapping(TextWrapping::Wrap)
                .font_weight(FontWeight::SEMI_BOLD)
                .horizontal_alignment(HorizontalAlignment::Center),
            TextBlock::new()
                .text(description)
                .text_trimming(TextTrimming::CharacterEllipsis)
                .opacity(0.7)
                .horizontal_alignment(HorizontalAlignment::Center),
            Border::new()
                .horizontal_alignment(HorizontalAlignment::Center)
                .margin(Thickness::new(0.0, 16.0, 0.0, 0.0))
                .content(action),
        ))
}

/// An icon with a label after it, for button content.
pub fn icon_label(symbol: Symbol, label: &str) -> View {
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(8.0)
        .children((SymbolIcon::new().symbol(symbol), label.to_owned()))
}

/// A title with smaller text under it.
pub fn title_and_subtitle(title: &str, subtitle_text: &str) -> View {
    let mut lines = vec![KeyedView::new(
        "title",
        TextBlock::new()
            .text(title)
            .text_trimming(TextTrimming::CharacterEllipsis),
    )];
    if !subtitle_text.is_empty() {
        lines.push(KeyedView::new(
            "subtitle",
            subtitle(subtitle_text).font_size(12.0),
        ));
    }
    StackPanel::new().keyed_children(lines)
}

/// A toggle switch without its "On"/"Off" label, and without the room it saves for one.
pub fn bare_switch(is_on: bool, on_toggled: Callback<bool>) -> View {
    ToggleSwitch::new()
        .is_on(is_on)
        .on_toggled(on_toggled)
        .min_width(0.0)
        .margin(Thickness::new(0., 0., -12., 0.))
        .slots([
            SlotView::new(ToggleSwitchSlot::OnContent, ""),
            SlotView::new(ToggleSwitchSlot::OffContent, ""),
        ])
}

pub fn row(left: impl Into<View>, right: impl Into<View>) -> View {
    Grid::new()
        .columns([GridLength::STAR, GridLength::Auto])
        .column_spacing(16.0)
        .children((
            Border::new()
                .vertical_alignment(VerticalAlignment::Center)
                .content(left),
            Border::new()
                .grid_column(1)
                .vertical_alignment(VerticalAlignment::Center)
                .content(right),
        ))
}

/// overflow button that opens a menu. the callback is passed the label of whatever was clicked.
pub fn menu_button(items: Vec<MenuItem>, on_click: Callback<String>) -> View {
    Button::new()
        .style(ButtonStyle::Subtle)
        .automation_name("More options")
        .content(SymbolIcon::new().symbol(Symbol::More))
        .menu(Menu::new(items, on_click))
}

pub fn menu_item(label: &str, enabled: bool) -> MenuItem {
    if enabled {
        MenuItem::item(label, label)
    } else {
        MenuItem::disabled(label, label)
    }
}

pub fn status_dot(running: bool) -> View {
    let color = if running {
        Color::rgb(0x2e, 0xa0, 0x43)
    } else {
        Color::rgb(0x8a, 0x8a, 0x8a)
    };

    Border::new()
        .width(8.0)
        .height(8.0)
        .corner_radius(4.0)
        .background(color)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
}

pub fn about_dialog(open: bool, on_closed: Callback<ContentDialogResult>) -> View {
    let content = StackPanel::new().spacing(8.0).children((
        TextBlock::new().text("Manage themes for GPUI apps running with erebus."),
        subtitle(format!("Version {}", env!("CARGO_PKG_VERSION"))),
        subtitle("© 2026 Tropical, Licensed under the MIT License"),
        HyperlinkButton::new()
            .navigate_uri(REPO)
            .unwrap_or_else(|_| HyperlinkButton::new())
            .content("Source code"),
    ));

    ContentDialog::new()
        .title("erebus-frontend-winui")
        .close_button_text("Close")
        .is_open(open)
        .on_closed(on_closed)
        .content(content)
}
