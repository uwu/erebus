//! The gradients here are SVGs because zgui's stylesheets can't draw them yet.

use std::{cell::Cell, rc::Rc, time::Instant};

use erebus_theme::primitives::Rgba;
use zgui::prelude::*;
use zgui::reactive::{LocalStorage, RwSignal, UnsyncCallback};
use zgui::style;
use zgui::vocab::PropValue;

/// Keep the picker's color in HSV so dragging through grey or black doesn't forget the selected
/// hue. Hue is in degrees; the other components use `0..=1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsva {
    pub h: f32,
    pub s: f32,
    pub v: f32,
    pub a: f32,
}

impl Hsva {
    pub fn to_rgba(self) -> Rgba {
        let h = self.h.rem_euclid(360.0) / 60.0;
        let c = self.v * self.s;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = self.v - c;
        Rgba {
            r: r + m,
            g: g + m,
            b: b + m,
            a: self.a,
        }
    }

    /// Grey has no hue to recover from RGB, so keep the previous `hue` in that case.
    pub fn from_rgba(color: Rgba, hue: f32) -> Self {
        let (r, g, b) = (color.r, color.g, color.b);
        let max = r.max(g).max(b);
        let delta = max - r.min(g).min(b);
        let h = if delta <= f32::EPSILON {
            hue
        } else if max == r {
            60.0 * ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        Self {
            h,
            s: if max <= f32::EPSILON {
                0.0
            } else {
                delta / max
            },
            v: max,
            a: color.a,
        }
    }

    fn opaque(self) -> Self {
        Self { a: 1.0, ..self }
    }
}

/// `#rrggbb`, ignoring alpha.
fn hex_rgb(color: Rgba) -> String {
    let [r, g, b] = [color.r, color.g, color.b].map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// We can throttle updates during a drag, but the final value always needs to get through.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Part of a drag or a held key; more are coming.
    Ongoing,
    /// Finished: a release, a click, or a single step.
    Done,
}

const AREA_WIDTH: f32 = 240.0;
const AREA_HEIGHT: f32 = 150.0;
const BAR_HEIGHT: f32 = 12.0;

style! { ColorPickerStyle => r#"
    :scope {
        gap: var(--zui-space-md);
        width: 240px;
    }

    :scope .area, :scope .bar {
        position: relative;
        width: 240px;
    }

    :scope .area {
        height: 150px;
        border-radius: 6px;
    }

    :scope .bar {
        height: 12px;
        border-radius: 6px;
    }

    :scope .fill {
        width: 100%;
        height: 100%;
        overflow: hidden;
    }

    :scope .thumb {
        position: absolute;
        width: 16px;
        height: 16px;
        margin-left: -8px;
        margin-top: -8px;
        border-radius: 8px;
        border: 2px solid white;
        box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.35), 0 1px 3px rgba(0, 0, 0, 0.4);
        pointer-events: none;
    }

    :scope .area:focus-visible .thumb,
     :scope .bar:focus-visible .thumb {
        box-shadow: 0 0 0 3px var(--zui-color-ring);
    }
"# }

/// Writes directly to `color`, then calls `on_change` after each update.
#[component]
pub fn ColorPicker(
    color: RwSignal<Hsva, LocalStorage>,
    on_change: UnsyncCallback<Change>,
) -> impl IntoView {
    let set = move |next: Hsva, change: Change| {
        color.set(next);
        on_change.run(change);
    };

    install_stylesheet(ColorPickerStyle::CLASS, ColorPickerStyle::CSS);
    view! {
        column(class = ColorPickerStyle::CLASS) {
            SaturationArea(color = color, set = UnsyncCallback::new(move |(next, change)| set(next, change)))
            HueBar(color = color, set = UnsyncCallback::new(move |(next, change)| set(next, change)))
            AlphaBar(color = color, set = UnsyncCallback::new(move |(next, change)| set(next, change)))
        }
    }
}

type Setter = UnsyncCallback<(Hsva, Change)>;

/// Convert window coordinates in CSS pixels to fractions of the node's width and height.
fn fractions(node: NodeRef, x: f32, y: f32) -> Option<(f32, f32)> {
    let bounds = node.window_bounds()?;
    let (width, height) = (bounds.size.width.0, bounds.size.height.0);
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let scale = node.scale();
    Some((
        ((x * scale - bounds.origin.x.0) / width).clamp(0.0, 1.0),
        ((y * scale - bounds.origin.y.0) / height).clamp(0.0, 1.0),
    ))
}

/// Holding Shift makes arrow keys take larger steps.
fn arrow_step(key: &Key, modifiers: Modifiers) -> Option<(f32, f32)> {
    let step = if modifiers.shift() { 0.1 } else { 0.01 };
    match key {
        Key::Named(NamedKey::ArrowLeft) => Some((-step, 0.0)),
        Key::Named(NamedKey::ArrowRight) => Some((step, 0.0)),
        Key::Named(NamedKey::ArrowUp) => Some((0.0, -step)),
        Key::Named(NamedKey::ArrowDown) => Some((0.0, step)),
        _ => None,
    }
}

/// Saturation across, value down.
#[component]
fn SaturationArea(color: RwSignal<Hsva, LocalStorage>, set: Setter) -> impl IntoView {
    let area = NodeRef::new();
    let dragging = RwSignal::new_local(false);
    let to = move |x: f32, y: f32, change: Change| {
        if let Some((fx, fy)) = fractions(area, x, y) {
            let current = color.get_untracked();
            set.run((
                Hsva {
                    s: fx,
                    v: 1.0 - fy,
                    ..current
                },
                change,
            ));
        }
    };

    let svg = move || {
        let hue = hex_rgb(
            Hsva {
                h: color.get().h,
                s: 1.0,
                v: 1.0,
                a: 1.0,
            }
            .to_rgba(),
        );
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}">
<defs>
<linearGradient id="white" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#fff"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></linearGradient>
<linearGradient id="black" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000"/></linearGradient>
</defs>
<rect width="{w}" height="{h}" rx="6" fill="{hue}"/>
<rect width="{w}" height="{h}" rx="6" fill="url(#white)"/>
<rect width="{w}" height="{h}" rx="6" fill="url(#black)"/>
</svg>"##,
            w = AREA_WIDTH,
            h = AREA_HEIGHT,
        )
    };

    view! {
        control(
            node_ref = area,
            class = "area",
            tabindex = Focus::Sequential,
            a11y:label = "Saturation and Brightness",
            on:pointer_down = move |ev| {
                ev.capture_pointer();
                dragging.set(true);
                to(ev.position.x.0, ev.position.y.0, Change::Ongoing);
            },
            on:pointer_move = move |ev| {
                if dragging.get_untracked() {
                    to(ev.position.x.0, ev.position.y.0, Change::Ongoing);
                }
            },
            on:pointer_up = move |ev| {
                if dragging.get_untracked() {
                    dragging.set(false);
                    ev.release_pointer();
                    to(ev.position.x.0, ev.position.y.0, Change::Done);
                }
            },
            on:pointer_cancel = move |ev| {
                dragging.set(false);
                ev.release_pointer();
            },
            on:key_down = move |ev| {
                if let Some((dx, dy)) = arrow_step(&ev.key, ev.modifiers) {
                    let current = color.get_untracked();
                    set.run((
                        Hsva {
                            s: (current.s + dx).clamp(0.0, 1.0),
                            v: (current.v - dy).clamp(0.0, 1.0),
                            ..current
                        },
                        Change::Done,
                    ));
                    ev.prevent_default();
                    ev.stop_propagation();
                }
            }
        ) {
            vector(class = "fill", prop:svg = move || PropValue::from(svg()), a11y:hidden = true)
            box(
                class = "thumb",
                style:left = move || Some(format!("{}%", color.get().s * 100.0)),
                style:top = move || Some(format!("{}%", (1.0 - color.get().v) * 100.0)),
                style:background-color = move || Some(hex_rgb(color.get().opaque().to_rgba()))
            )
        }
    }
}

#[component]
fn Bar(
    label: &'static str,
    /// the track, as SVG
    svg: Signal<String, LocalStorage>,
    /// where the thumb is, `0..=1`
    at: Signal<f32, LocalStorage>,
    thumb: Signal<String, LocalStorage>,
    /// Receives the new position as a fraction of the track.
    moved: UnsyncCallback<(f32, Change)>,
) -> impl IntoView {
    let track = NodeRef::new();
    let dragging = RwSignal::new_local(false);
    let to = move |x: f32, y: f32, change: Change| {
        if let Some((fx, _)) = fractions(track, x, y) {
            moved.run((fx, change));
        }
    };

    view! {
        control(
            node_ref = track,
            class = "bar",
            tabindex = Focus::Sequential,
            a11y:label = label,
            on:pointer_down = move |ev| {
                ev.capture_pointer();
                dragging.set(true);
                to(ev.position.x.0, ev.position.y.0, Change::Ongoing);
            },
            on:pointer_move = move |ev| {
                if dragging.get_untracked() {
                    to(ev.position.x.0, ev.position.y.0, Change::Ongoing);
                }
            },
            on:pointer_up = move |ev| {
                if dragging.get_untracked() {
                    dragging.set(false);
                    ev.release_pointer();
                    to(ev.position.x.0, ev.position.y.0, Change::Done);
                }
            },
            on:pointer_cancel = move |ev| {
                dragging.set(false);
                ev.release_pointer();
            },
            on:key_down = move |ev| {
                if let Some((dx, _)) = arrow_step(&ev.key, ev.modifiers).filter(|(dx, _)| *dx != 0.0) {
                    moved.run(((at.get_untracked() + dx).clamp(0.0, 1.0), Change::Done));
                    ev.prevent_default();
                    ev.stop_propagation();
                }
            }
        ) {
            vector(class = "fill", prop:svg = move || PropValue::from(svg.get()), a11y:hidden = true)
            box(
                class = "thumb",
                style:left = move || Some(format!("{}%", at.get() * 100.0)),
                style:top = Some("50%".to_owned()),
                style:background-color = move || Some(thumb.get())
            )
        }
    }
}

#[component]
fn HueBar(color: RwSignal<Hsva, LocalStorage>, set: Setter) -> impl IntoView {
    let stops = (0..=6)
        .map(|i| {
            let rgba = Hsva {
                h: i as f32 * 60.0,
                s: 1.0,
                v: 1.0,
                a: 1.0,
            }
            .to_rgba();
            format!(
                r#"<stop offset="{}" stop-color="{}"/>"#,
                i as f32 / 6.0,
                hex_rgb(rgba)
            )
        })
        .collect::<String>();
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}"><defs><linearGradient id="hue" x1="0" y1="0" x2="1" y2="0">{stops}</linearGradient></defs><rect width="{w}" height="{h}" rx="{r}" fill="url(#hue)"/></svg>"##,
        w = AREA_WIDTH,
        h = BAR_HEIGHT,
        r = BAR_HEIGHT / 2.0,
    );

    view! {
        Bar(
            label = "Hue",
            svg = Signal::stored_local(svg),
            at = Signal::derive_local(move || color.get().h / 360.0),
            thumb = Signal::derive_local(move || {
                hex_rgb(Hsva { s: 1.0, v: 1.0, ..color.get() }.opaque().to_rgba())
            }),
            moved = UnsyncCallback::new(move |(fraction, change): (f32, Change)| {
                // just short of 360, so the end of the bar stays at the end rather than wrapping
                let h = (fraction * 360.0).min(359.9);
                set.run((Hsva { h, ..color.get_untracked() }, change));
            })
        )
    }
}

#[component]
fn AlphaBar(color: RwSignal<Hsva, LocalStorage>, set: Setter) -> impl IntoView {
    let svg = move || {
        let solid = hex_rgb(color.get().opaque().to_rgba());
        let size = BAR_HEIGHT / 2.0;
        let checks = (0..(AREA_WIDTH / size) as u32)
            .flat_map(|column| {
                (0..2).filter_map(move |row| {
                    ((column + row) % 2 == 0).then(|| {
                        format!(
                            r#"<rect x="{}" y="{}" width="{size}" height="{size}"/>"#,
                            column as f32 * size,
                            row as f32 * size
                        )
                    })
                })
            })
            .collect::<String>();
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}"><defs><clipPath id="round"><rect width="{w}" height="{h}" rx="{r}"/></clipPath><linearGradient id="alpha" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="{solid}" stop-opacity="0"/><stop offset="1" stop-color="{solid}"/></linearGradient></defs><g clip-path="url(#round)"><rect width="{w}" height="{h}" fill="#fff"/><g fill="#ccc">{checks}</g><rect width="{w}" height="{h}" fill="url(#alpha)"/></g></svg>"##,
            w = AREA_WIDTH,
            h = BAR_HEIGHT,
            r = BAR_HEIGHT / 2.0,
        )
    };

    view! {
        Bar(
            label = "Opacity",
            svg = Signal::derive_local(svg),
            at = Signal::derive_local(move || color.get().a),
            thumb = Signal::derive_local(move || hex_rgb(color.get().opaque().to_rgba())),
            moved = UnsyncCallback::new(move |(fraction, change): (f32, Change)| {
                set.run((Hsva { a: fraction, ..color.get_untracked() }, change));
            })
        )
    }
}

/// Each update saves the library and notifies the app. Limit those to once every 50ms while
/// dragging, but always send the final value.
pub fn throttled(emit: impl Fn() + 'static) -> impl Fn(Change) + 'static {
    let last = Rc::new(Cell::new(None::<Instant>));
    move |change| {
        let now = Instant::now();
        let due = last
            .get()
            .is_none_or(|at| now.duration_since(at).as_millis() >= 50);
        if change == Change::Done || due {
            last.set(Some(now));
            emit();
        }
    }
}
