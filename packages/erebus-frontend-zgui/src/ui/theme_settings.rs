//! Controls for a theme's settings.

use std::rc::Rc;

use crate::components::tooltip::StyledTooltipProps;
use crate::components::{
    color_picker::{Change, ColorPickerProps, Hsva, throttled},
    octicons::{DASH, PLUS},
};
use erebus_theme::{
    primitives::Rgba,
    settings::{OwnedSettingKind, OwnedSettingsField, SettingValue},
};
use zgui::prelude::*;
use zgui::reactive::{RwSignal, UnsyncCallback};
use zgui::style;
use zgui_ui::prelude::*;
use zgui_ui_icons::prelude::*;

type OnChange = Rc<dyn Fn(SettingValue)>;

style! { NumberStyle => r#"
    :scope {
        width: 128px;
    }
"# }

style! { ColorStyle => r#"
    :scope {
        align-items: center;
        gap: var(--zui-space-sm);
    }

    :scope .swatch {
        width: 100%;
        height: 100%;
        border-radius: calc(var(--zui-radius-md) - 2px);
        border: 1px solid var(--zui-color-border);
    }

    /* doubled to outrank the input's own sans font */
    :scope .input.input {
        font-family: var(--zui-type-family-mono);
    }
"# }

// The popover renders in the overlay, so attach these styles to its surface.
style! { PopoverStyle => r#"
    :scope:scope {
        width: auto;
        padding: var(--zui-space-md);
    }
"# }

#[component]
pub fn SettingRow(
    field: OwnedSettingsField,
    current: Option<SettingValue>,
    on_change: OnChange,
) -> impl IntoView {
    let OwnedSettingsField {
        label,
        description,
        kind,
        ..
    } = field;
    // a stored value of another kind is from an older version of the theme
    let current = current
        .filter(|value| same_kind(value, &kind))
        .unwrap_or_else(|| kind.default_value());

    let control = match (kind, current) {
        (OwnedSettingKind::Bool { .. }, SettingValue::Bool(value)) => {
            let on = RwSignal::new_local(value);
            let checked =
                Binding::controlled(Signal::derive_local(move || on.get()), move |value| {
                    on.set(value);
                    on_change(SettingValue::Bool(value));
                });
            AnyView::new(view! { Switch(checked = checked, {..use_settings_item_attrs()}) })
        }

        (OwnedSettingKind::Int { min, max, .. }, SettingValue::Int(value)) => AnyView::new(view! {
            NumberField(
                value = value as f64,
                min = min as f64,
                max = max as f64,
                step = 1.0,
                digits = 0,
                on_change = Rc::new(move |value: f64| on_change(SettingValue::Int(value.round() as i64)))
            )
        }),

        (OwnedSettingKind::Float { min, max, step, .. }, SettingValue::Float(value)) => {
            let step = if step > 0.0 { step } else { 0.01 };
            AnyView::new(view! {
                NumberField(
                    value = value,
                    min = min,
                    max = max,
                    step = step,
                    digits = (-step.log10()).ceil().clamp(0.0, 6.0) as usize,
                    on_change = Rc::new(move |value: f64| on_change(SettingValue::Float(value)))
                )
            })
        }

        (OwnedSettingKind::Color { .. }, SettingValue::Color(value)) => {
            AnyView::new(view! { ColorField(value = value, on_change = on_change) })
        }

        (OwnedSettingKind::Choice { options, .. }, SettingValue::Choice(value)) => {
            let selected = RwSignal::new_local(value);
            let choice = Binding::controlled(
                Signal::derive_local(move || selected.get()),
                move |value: String| {
                    selected.set(value.clone());
                    on_change(SettingValue::Choice(value));
                },
            );

            AnyView::new(view! {
                Select(value = choice) {
                    SelectTrigger(size = SelectTriggerSize::Sm, {..use_settings_item_attrs()}) {
                        SelectValue()
                    }
                    SelectContent {
                        {options
                            .iter()
                            .map(|option| {
                                let (value, label) = (option.value.clone(), option.label.clone());
                                view! { SelectItem(value = value) {{label}} }.into_view()
                            })
                            .collect::<Vec<_>>()}
                    }
                }
            })
        }

        _ => unreachable!("the value was checked against its kind"),
    };

    AnyView::new(view! {
        SettingsItem(
            label = label,
            description = (!description.is_empty()).then_some(description)
        ) {
            {control}
        }
    })
}

#[component]
fn NumberField(
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    digits: usize,
    on_change: Rc<dyn Fn(f64)>,
) -> impl IntoView {
    let format = move |value: f64| format!("{value:.digits$}");
    let number = RwSignal::new_local(value);
    let text = RwSignal::new_local(format(value));
    let invalid = RwSignal::new_local(false);

    let commit = {
        let on_change = on_change.clone();
        move |value: f64| {
            let value = value.clamp(min, max);
            number.set(value);
            invalid.set(false);
            on_change(value);
        }
    };
    let typed = {
        let commit = commit.clone();
        UnsyncCallback::new(move |typed: String| {
            match typed.trim().parse::<f64>() {
                Ok(value) if (min..=max).contains(&value) => commit(value),
                _ => invalid.set(true),
            }
            text.set(typed);
        })
    };
    let stepper = move |by: f64| {
        let commit = commit.clone();
        move |_: &mut EventCx<'_, zgui::view::events::Click>| {
            let value = (number.get_untracked() + by).clamp(min, max);
            text.set(format(value));
            commit(value);
        }
    };
    let (decrease, increase) = (stepper(-step), stepper(step));
    install_stylesheet(NumberStyle::CLASS, NumberStyle::CSS);

    view! {
        InputGroup(class = NumberStyle::CLASS) {
            InputGroupAddon {
                StyledTooltip(label = "Decrease") {
                    InputGroupButton(
                        size = InputGroupButtonSize::IconXs,
                        disabled = Signal::derive_local(move || number.get() <= min),
                        a11y:label = "Decrease",
                        on:click = decrease
                    ) {
                        Icon(icon = DASH, label = "")
                    }
                }
            }
            InputGroupInput(value = text, on_change = typed, invalid = invalid, {..use_settings_item_attrs()})
            InputGroupAddon(align = InputGroupAddonAlign::InlineEnd) {
                StyledTooltip(label = "Increase") {
                    InputGroupButton(
                        size = InputGroupButtonSize::IconXs,
                        disabled = Signal::derive_local(move || number.get() >= max),
                        a11y:label = "Increase",
                        on:click = increase
                    ) {
                        Icon(icon = PLUS, label = "")
                    }
                }
            }
        }
    }
}

/// Accept both `#rrggbb` and `#rrggbbaa`.
#[component]
fn ColorField(value: Rgba, on_change: OnChange) -> impl IntoView {
    let picked = RwSignal::new_local(Hsva::from_rgba(value, 0.0));
    let text = RwSignal::new_local(to_hex(value));
    let invalid = RwSignal::new_local(false);

    // Changes from the picker need to update the text field too.
    let emit = {
        let on_change = on_change.clone();
        throttled(move || on_change(SettingValue::Color(picked.get_untracked().to_rgba())))
    };
    let from_picker = UnsyncCallback::new(move |change: Change| {
        text.set(to_hex(picked.get_untracked().to_rgba()));
        invalid.set(false);
        emit(change);
    });

    let typed = UnsyncCallback::new(move |typed: String| {
        match from_hex(&typed) {
            Some(value) => {
                picked.set(Hsva::from_rgba(value, picked.get_untracked().h));
                invalid.set(false);
                on_change(SettingValue::Color(value));
            }
            None => invalid.set(true),
        }
        text.set(typed);
    });

    install_stylesheet(ColorStyle::CLASS, ColorStyle::CSS);
    install_stylesheet(PopoverStyle::CLASS, PopoverStyle::CSS);
    view! {
        row(class = ColorStyle::CLASS) {
            StyledTooltip(label = "Pick a Color") {
                Popover {
                    PopoverTrigger(
                        variant = ButtonVariant::Outline,
                        size = ButtonSize::Icon,
                        a11y:label = "Pick a Color"
                    ) {
                        box(
                            class = "swatch",
                            style:background-color = move || Some(css_color(picked.get().to_rgba()))
                        )
                    }
                    PopoverContent(class = PopoverStyle::CLASS) {
                        ColorPicker(color = picked, on_change = from_picker)
                    }
                }
            }
            Input(
                class = "input",
                value = text,
                on_change = typed,
                invalid = invalid,
                {..use_settings_item_attrs()}
            )
        }
    }
}

fn same_kind(value: &SettingValue, kind: &OwnedSettingKind) -> bool {
    matches!(
        (value, kind),
        (SettingValue::Bool(_), OwnedSettingKind::Bool { .. })
            | (SettingValue::Int(_), OwnedSettingKind::Int { .. })
            | (SettingValue::Float(_), OwnedSettingKind::Float { .. })
            | (SettingValue::Color(_), OwnedSettingKind::Color { .. })
            | (SettingValue::Choice(_), OwnedSettingKind::Choice { .. })
    )
}

fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn to_hex(color: Rgba) -> String {
    let [r, g, b, a] = [color.r, color.g, color.b, color.a].map(channel);
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

fn from_hex(text: &str) -> Option<Rgba> {
    let hex = text.trim().strip_prefix('#').unwrap_or(text.trim());
    if !matches!(hex.len(), 6 | 8) || !hex.is_ascii() {
        return None;
    }

    let byte = |index: usize| {
        hex.get(index * 2..index * 2 + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .map(|byte| f32::from(byte) / 255.0)
    };
    Some(Rgba {
        r: byte(0)?,
        g: byte(1)?,
        b: byte(2)?,
        a: if hex.len() == 8 { byte(3)? } else { 1.0 },
    })
}

fn css_color(color: Rgba) -> String {
    let [r, g, b] = [color.r, color.g, color.b].map(channel);
    format!("rgba({r}, {g}, {b}, {})", color.a.clamp(0.0, 1.0))
}
