//! Controls for a theme's settings.

use erebus_theme::{
    primitives::Rgba,
    settings::{OwnedSettingKind, OwnedSettingsField, SettingValue},
};
use windows_reactor::*;

use crate::ui::{Erebus, Message, widgets};

/// A row for `field`. `set` turns each new value into the message that saves it.
pub fn setting_row(
    field: &OwnedSettingsField,
    current: Option<&SettingValue>,
    set: impl Fn(SettingValue) -> Message + Clone + 'static,
    context: &ViewContext<Erebus>,
) -> View {
    let control: View = match &field.kind {
        OwnedSettingKind::Bool { default } => widgets::bare_switch(
            match current {
                Some(SettingValue::Bool(value)) => *value,
                _ => *default,
            },
            context.callback(move |on| set(SettingValue::Bool(on))),
        ),

        OwnedSettingKind::Int { default, min, max } => NumberBox::new()
            .minimum(*min as f64)
            .maximum(*max as f64)
            .value(match current {
                Some(SettingValue::Int(value)) => *value,
                _ => *default,
            } as f64)
            .on_value_changed(context.callback(move |value: Option<f64>| {
                set(SettingValue::Int(value.unwrap_or_default().round() as i64))
            }))
            .into(),

        OwnedSettingKind::Float {
            default, min, max, ..
        } => NumberBox::new()
            .minimum(*min)
            .maximum(*max)
            .value(match current {
                Some(SettingValue::Float(value)) => *value,
                _ => *default,
            })
            .on_value_changed(context.callback(move |value: Option<f64>| {
                set(SettingValue::Float(value.unwrap_or_default()))
            }))
            .into(),

        OwnedSettingKind::Color { default } => {
            let color = to_color(match current {
                Some(SettingValue::Color(value)) => *value,
                _ => *default,
            });

            let picker = ColorPicker::new()
                .color(color)
                .is_alpha_enabled(true)
                .on_color_changed(
                    context.callback(move |color| set(SettingValue::Color(from_color(color)))),
                );

            Button::new()
                .automation_name("Pick a color")
                .content(
                    Border::new()
                        .width(24.0)
                        .height(24.0)
                        .corner_radius(4.0)
                        .border_thickness(1.0)
                        .border_brush(ThemeBrush::CardStroke)
                        .background(color),
                )
                .flyout_with(Flyout::rich(picker).placement(FlyoutPlacement::Bottom))
        }

        OwnedSettingKind::Choice { default, options } => {
            let selected = match current {
                Some(SettingValue::Choice(value)) => value,
                _ => default,
            };

            let values: Vec<String> = options.iter().map(|option| option.value.clone()).collect();
            ComboBox::new()
                .items_source(options.iter().map(|option| option.label.clone()))
                .selected_index(options.iter().position(|option| &option.value == selected))
                .on_selection_changed(context.callback(move |index: Option<usize>| {
                    let value = index.and_then(|index| values.get(index)).or(values.first());
                    set(SettingValue::Choice(value.cloned().unwrap_or_default()))
                }))
                .into()
        }
    };

    widgets::row(
        widgets::title_and_subtitle(&field.label, &field.description),
        control,
    )
}

fn to_color(rgba: Rgba) -> Color {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::argb(
        channel(rgba.a),
        channel(rgba.r),
        channel(rgba.g),
        channel(rgba.b),
    )
}

fn from_color(color: Color) -> Rgba {
    let channel = |value: u8| value as f32 / 255.0;
    Rgba {
        r: channel(color.r),
        g: channel(color.g),
        b: channel(color.b),
        a: channel(color.a),
    }
}
