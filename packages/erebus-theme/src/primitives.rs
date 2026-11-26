//! GPUI Scene Primitives
//!
//! This module re-creates most `#[repr(C)]` primitive structs and subtypes from
//! <https://github.com/zed-industries/zed/blob/main/crates/gpui/src/scene.rs>.
//!
//! Every type here must exactly match the layout of GPUI's struct layouts.

// Portions of this file are derived from the zed editor, which is licensed
// under the Apache License, Version 2.0.
//
// Copyright 2022 - 2025 Zed Industries, Inc.
//
// <https://github.com/zed-industries/zed/blob/main/LICENSE-APACHE>

use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter},
    hash::{Hash, Hasher},
};

// Color behavior follows GPUI color.rs; serde retains Erebus's four-float wire format.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Hsla {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}

impl Hash for Hsla {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(u32::from_be_bytes(self.h.to_be_bytes()));
        state.write_u32(u32::from_be_bytes(self.s.to_be_bytes()));
        state.write_u32(u32::from_be_bytes(self.l.to_be_bytes()));
        state.write_u32(u32::from_be_bytes(self.a.to_be_bytes()));
    }
}

impl Display for Hsla {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "hsla({:.2}, {:.2}%, {:.2}%, {:.2})",
            self.h * 360.,
            self.s * 100.,
            self.l * 100.,
            self.a
        )
    }
}

impl Hsla {
    pub const WHITE: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 1.0,
    };

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::from_rgba(r, g, b, 255)
    }

    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::from_rgba_f32(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    /// Converts straight sRGB components in `0.0..=1.0` to HSLA.
    pub const fn from_rgba_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        let max = r.max(g.max(b));
        let min = r.min(g.min(b));
        let delta = max - min;

        let l = (max + min) / 2.0;

        // Keep the same arithmetic as GPUI's runtime conversion. The equivalent
        // 1 - abs(2*l - 1) form loses precision for dark colors (and can round
        // to zero), causing const tokens to differ from incoming paint colors.
        let s = if l == 0.0 || l == 1.0 {
            0.0
        } else if l < 0.5 {
            delta / (2.0 * l)
        } else {
            delta / (2.0 - 2.0 * l)
        };

        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            // Const equivalent of GPUI's rem_euclid(6.0).
            let h = ((g - b) / delta) % 6.0;
            (if h < 0.0 { h + 6.0 } else { h }) / 6.0
        } else if max == g {
            ((b - r) / delta + 2.0) / 6.0
        } else {
            ((r - g) / delta + 4.0) / 6.0
        };

        Hsla { h, s, l, a }
    }

    pub fn approx_eq(self, other: Hsla) -> bool {
        const EPSILON: f32 = 1.0 / 512.0;
        (self.h - other.h).abs() < EPSILON
            && (self.s - other.s).abs() < EPSILON
            && (self.l - other.l).abs() < EPSILON
            && (self.a - other.a).abs() < EPSILON
    }
}

/// Construct an [`Hsla`] object from plain values
pub const fn hsla(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    Hsla {
        h: h.clamp(0., 1.),
        s: s.clamp(0., 1.),
        l: l.clamp(0., 1.),
        a: a.clamp(0., 1.),
    }
}

/// Pure black in [`Hsla`]
pub const fn black() -> Hsla {
    Hsla {
        h: 0.,
        s: 0.,
        l: 0.,
        a: 1.,
    }
}

/// Transparent black in [`Hsla`]
pub const fn transparent_black() -> Hsla {
    Hsla {
        h: 0.,
        s: 0.,
        l: 0.,
        a: 0.,
    }
}

/// Transparent white in [`Hsla`]
pub const fn transparent_white() -> Hsla {
    Hsla {
        h: 0.,
        s: 0.,
        l: 1.,
        a: 0.,
    }
}

/// Opaque grey in [`Hsla`], values will be clamped to the range [0, 1]
pub const fn opaque_grey(lightness: f32, opacity: f32) -> Hsla {
    Hsla {
        h: 0.,
        s: 0.,
        l: lightness.clamp(0., 1.),
        a: opacity.clamp(0., 1.),
    }
}

/// Pure white in [`Hsla`]
pub const fn white() -> Hsla {
    Hsla {
        h: 0.,
        s: 0.,
        l: 1.,
        a: 1.,
    }
}

/// The color red in [`Hsla`]
pub const fn red() -> Hsla {
    Hsla {
        h: 0.,
        s: 1.,
        l: 0.5,
        a: 1.,
    }
}

/// The color blue in [`Hsla`]
pub const fn blue() -> Hsla {
    Hsla {
        h: 0.666_666_7,
        s: 1.,
        l: 0.5,
        a: 1.,
    }
}

/// The color green in [`Hsla`]
pub const fn green() -> Hsla {
    Hsla {
        h: 0.333_333_34,
        s: 1.,
        l: 0.25,
        a: 1.,
    }
}

/// The color yellow in [`Hsla`]
pub const fn yellow() -> Hsla {
    Hsla {
        h: 0.166_666_67,
        s: 1.,
        l: 0.5,
        a: 1.,
    }
}

impl Hsla {
    /// Converts this HSLA color to an RGBA color.
    pub fn to_rgb(self) -> Rgba {
        self.into()
    }

    /// The color red
    pub const fn red() -> Self {
        red()
    }

    /// The color green
    pub const fn green() -> Self {
        green()
    }

    /// The color blue
    pub const fn blue() -> Self {
        blue()
    }

    /// The color black
    pub const fn black() -> Self {
        black()
    }

    /// The color white
    pub const fn white() -> Self {
        white()
    }

    /// The color transparent black
    pub const fn transparent_black() -> Self {
        transparent_black()
    }

    /// Returns true if the HSLA color is fully transparent, false otherwise.
    pub fn is_transparent(&self) -> bool {
        self.a == 0.0
    }

    /// Returns true if the HSLA color is fully opaque, false otherwise.
    pub fn is_opaque(&self) -> bool {
        self.a == 1.0
    }

    /// Blends `other` on top of `self` based on `other`'s alpha value. The resulting color is a combination of `self`'s and `other`'s colors.
    ///
    /// If `other`'s alpha value is 1.0 or greater, `other` color is fully opaque, thus `other` is returned as the output color.
    /// If `other`'s alpha value is 0.0 or less, `other` color is fully transparent, thus `self` is returned as the output color.
    /// Else, the output color is calculated as a blend of `self` and `other` based on their weighted alpha values.
    ///
    /// Assumptions:
    /// - Alpha values are contained in the range [0, 1], with 1 as fully opaque and 0 as fully transparent.
    /// - The relative contributions of `self` and `other` is based on `self`'s alpha value (`self.a`) and `other`'s  alpha value (`other.a`), `self` contributing `self.a * (1.0 - other.a)` and `other` contributing its own alpha value.
    /// - RGB color components are contained in the range [0, 1].
    /// - If `self` and `other` colors are out of the valid range, the blend operation's output and behavior is undefined.
    pub fn blend(self, other: Hsla) -> Hsla {
        let alpha = other.a;

        if alpha >= 1.0 {
            other
        } else if alpha <= 0.0 {
            self
        } else {
            let converted_self = Rgba::from(self);
            let converted_other = Rgba::from(other);
            let blended_rgb = converted_self.blend(converted_other);
            Hsla::from(blended_rgb)
        }
    }

    /// Returns a new HSLA color with the same hue, and lightness, but with no saturation.
    pub fn grayscale(&self) -> Self {
        Hsla {
            h: self.h,
            s: 0.,
            l: self.l,
            a: self.a,
        }
    }

    /// Returns a color with `amount` added to its lightness.
    ///
    /// The amount and resulting lightness are clamped to `0.0..=1.0`.
    /// For example, `lighten(0.1)` increases lightness by 10 percentage points.
    /// Hue, saturation, and alpha are preserved.
    pub const fn lighten(&self, amount: f32) -> Self {
        Self {
            l: (self.l + amount.clamp(0.0, 1.0)).clamp(0.0, 1.0),
            ..*self
        }
    }

    /// Returns a color with `amount` subtracted from its lightness.
    ///
    /// The amount and resulting lightness are clamped to `0.0..=1.0`.
    /// For example, `darken(0.1)` decreases lightness by 10 percentage points.
    /// Hue, saturation, and alpha are preserved.
    pub const fn darken(&self, amount: f32) -> Self {
        Self {
            l: (self.l - amount.clamp(0.0, 1.0)).clamp(0.0, 1.0),
            ..*self
        }
    }

    /// Fade out the color by a given factor. This factor should be between 0.0 and 1.0.
    /// Where 0.0 will leave the color unchanged, and 1.0 will completely fade out the color.
    pub fn fade_out(&mut self, factor: f32) {
        self.a *= 1.0 - factor.clamp(0., 1.);
    }

    /// Multiplies the alpha value of the color by a given factor
    /// and returns a new HSLA color.
    ///
    /// Useful for transforming colors with dynamic opacity,
    /// like a color from an external source.
    ///
    /// Example:
    /// ```
    /// let color = erebus_theme::primitives::red();
    /// let faded_color = color.opacity(0.5);
    /// assert_eq!(faded_color.a, 0.5);
    /// ```
    ///
    /// This will return a red color with half the opacity.
    ///
    /// Example:
    /// ```
    /// use erebus_theme::primitives::hsla;
    /// let color = hsla(0.7, 1.0, 0.5, 0.7); // A saturated blue
    /// let faded_color = color.opacity(0.16);
    /// assert!((faded_color.a - 0.112).abs() < 1e-6);
    /// ```
    ///
    /// This will return a blue color with around ~10% opacity,
    /// suitable for an element's hover or selected state.
    ///
    pub fn opacity(&self, factor: f32) -> Self {
        Hsla {
            h: self.h,
            s: self.s,
            l: self.l,
            a: self.a * factor.clamp(0., 1.),
        }
    }

    /// Returns a new HSLA color with the same hue, saturation,
    /// and lightness, but with a new alpha value.
    ///
    /// Example:
    /// ```
    /// let color = erebus_theme::primitives::red();
    /// let red_color = color.alpha(0.25);
    /// assert_eq!(red_color.a, 0.25);
    /// ```
    ///
    /// This will return a red color with 25% opacity.
    ///
    /// Example:
    /// ```
    /// use erebus_theme::primitives::hsla;
    /// let color = hsla(0.7, 1.0, 0.5, 0.7); // A saturated blue
    /// let faded_color = color.alpha(0.25);
    /// assert_eq!(faded_color.a, 0.25);
    /// ```
    ///
    /// This will return a blue color with 25% opacity.
    pub fn alpha(&self, a: f32) -> Self {
        Hsla {
            h: self.h,
            s: self.s,
            l: self.l,
            a: a.clamp(0., 1.),
        }
    }
}

impl From<Rgba> for Hsla {
    fn from(color: Rgba) -> Self {
        Self::from_rgba_f32(color.r, color.g, color.b, color.a)
    }
}

/// An RGBA color
#[derive(PartialEq, Clone, Copy, Default, Serialize, Deserialize)]
#[repr(C)]
pub struct Rgba {
    /// The red component of the color, in the range 0.0 to 1.0
    pub r: f32,
    /// The green component of the color, in the range 0.0 to 1.0
    pub g: f32,
    /// The blue component of the color, in the range 0.0 to 1.0
    pub b: f32,
    /// The alpha component of the color, in the range 0.0 to 1.0
    pub a: f32,
}

impl core::fmt::Debug for Rgba {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "rgba({:#010x})", u32::from(*self))
    }
}

impl Rgba {
    /// Create a new [`Rgba`] color by blending this and another color together
    pub fn blend(&self, other: Rgba) -> Self {
        if other.a >= 1.0 {
            other
        } else if other.a <= 0.0 {
            *self
        } else {
            Rgba {
                r: (self.r * (1.0 - other.a)) + (other.r * other.a),
                g: (self.g * (1.0 - other.a)) + (other.g * other.a),
                b: (self.b * (1.0 - other.a)) + (other.b * other.a),
                a: self.a,
            }
        }
    }

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: (r as f32) / 255.0,
            g: (g as f32) / 255.0,
            b: (b as f32) / 255.0,
            a: 1.0,
        }
    }
}

impl From<Hsla> for Rgba {
    fn from(color: Hsla) -> Self {
        let h = color.h;
        let s = color.s;
        let l = color.l;

        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
        let m = l - c / 2.0;
        let cm = c + m;
        let xm = x + m;

        let (r, g, b) = match (h * 6.0).floor() as i32 {
            0 | 6 => (cm, xm, m),
            1 => (xm, cm, m),
            2 => (m, cm, xm),
            3 => (m, xm, cm),
            4 => (xm, m, cm),
            _ => (cm, m, xm),
        };

        Rgba {
            r: r.clamp(0., 1.),
            g: g.clamp(0., 1.),
            b: b.clamp(0., 1.),
            a: color.a,
        }
    }
}

impl From<Rgba> for u32 {
    fn from(rgba: Rgba) -> Self {
        let r = (rgba.r * 255.0) as u32;
        let g = (rgba.g * 255.0) as u32;
        let b = (rgba.b * 255.0) as u32;
        let a = (rgba.a * 255.0) as u32;
        (r << 24) | (g << 16) | (b << 8) | a
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub origin: Point,
    pub size: Size,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ContentMask {
    pub bounds: Bounds,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundTag(pub u32);
impl BackgroundTag {
    pub const SOLID: Self = Self(0);
    pub const LINEAR_GRADIENT: Self = Self(1);
    pub const PATTERN_SLASH: Self = Self(2);
    pub const CHECKERBOARD: Self = Self(3);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorSpace(pub u32);
impl ColorSpace {
    pub const SRGB: Self = Self(0);
    pub const OKLAB: Self = Self(1);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorderStyle(pub u32);
impl BorderStyle {
    pub const SOLID: Self = Self(0);
    pub const DASHED: Self = Self(1);
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LinearColorStop {
    pub color: Hsla,
    pub percentage: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Background {
    pub tag: BackgroundTag,
    pub color_space: ColorSpace,
    pub solid: Hsla,
    pub gradient_angle_or_pattern_height: f32,
    pub colors: [LinearColorStop; 2],
    pub pad: u32,
}

impl Background {
    pub fn solid(color: Hsla) -> Self {
        Self {
            tag: BackgroundTag::SOLID,
            solid: color,
            ..Self::default()
        }
    }

    pub fn linear_gradient(
        angle_degrees: f32,
        from: LinearColorStop,
        to: LinearColorStop,
        color_space: ColorSpace,
    ) -> Self {
        Self {
            tag: BackgroundTag::LINEAR_GRADIENT,
            color_space,
            gradient_angle_or_pattern_height: angle_degrees,
            colors: [from, to],
            ..Self::default()
        }
    }

    pub fn solid_color(&self) -> Option<Hsla> {
        (self.tag == BackgroundTag::SOLID).then_some(self.solid)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaddedBool32(pub u32);

impl PaddedBool32 {
    pub const FALSE: Self = Self(0);
    pub const TRUE: Self = Self(1);

    pub const fn get(self) -> bool {
        self.0 != 0
    }
}

impl From<bool> for PaddedBool32 {
    fn from(value: bool) -> Self {
        Self(value as u32)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevicePoint {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSize {
    pub width: i32,
    pub height: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceBounds {
    pub origin: DevicePoint,
    pub size: DeviceSize,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasTextureKind(pub u32);
impl AtlasTextureKind {
    pub const MONOCHROME: Self = Self(0);
    pub const POLYCHROME: Self = Self(1);
    pub const SUBPIXEL: Self = Self(2);
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasTextureId {
    pub index: u32,
    pub kind: AtlasTextureKind,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileId(pub u32);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasTile {
    pub texture_id: AtlasTextureId,
    pub tile_id: TileId,
    pub padding: u32,
    pub bounds: DeviceBounds,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformationMatrix {
    pub rotation_scale: [[f32; 2]; 2],
    pub translation: [f32; 2],
}

impl TransformationMatrix {
    pub const fn unit() -> Self {
        Self {
            rotation_scale: [[1.0, 0.0], [0.0, 1.0]],
            translation: [0.0, 0.0],
        }
    }
}

impl Default for TransformationMatrix {
    fn default() -> Self {
        Self::unit()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PolychromeSprite {
    pub order: u32,
    pub pad: u32,
    pub tile: AtlasTile,
    pub grayscale: PaddedBool32,
    pub opacity: f32,
    pub bounds: Bounds,
    pub content_mask: ContentMask,
    pub corner_radii: Corners,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SubpixelSprite {
    pub order: u32,
    /// align to 8 bytes
    pub pad: u32,
    pub tile: AtlasTile,
    pub bounds: Bounds,
    pub content_mask: ContentMask,
    pub color: Hsla,
    pub transformation: TransformationMatrix,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MonochromeSprite {
    pub order: u32,
    pub pad: u32,
    pub tile: AtlasTile,
    pub bounds: Bounds,
    pub content_mask: ContentMask,
    pub color: Hsla,
    pub transformation: TransformationMatrix,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub order: u32,
    pub blur_radius: f32,
    pub bounds: Bounds,
    pub corner_radii: Corners,
    pub content_mask: ContentMask,
    pub color: Hsla,
    pub element_bounds: Bounds,
    pub element_corner_radii: Corners,
    /// 0 = drop shadow (rendered outside the element), 1 = inset shadow (rendered inside).
    pub inset: u32,
    pub pad: u32, // align to 8 bytes
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Underline {
    pub order: u32,
    pub pad: u32, // align to 8 bytes
    pub bounds: Bounds,
    pub content_mask: ContentMask,
    pub color: Hsla,
    pub thickness: f32,
    pub wavy: PaddedBool32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Quad {
    pub order: u32,
    pub border_style: BorderStyle,
    pub bounds: Bounds,
    pub content_mask: ContentMask,
    pub background: Background,
    pub border_color: Hsla,
    pub corner_radii: Corners,
    pub border_widths: Edges,
}

pub trait Primitive: Copy + PartialEq {}

impl Primitive for Quad {}
impl Primitive for Shadow {}
impl Primitive for Underline {}
impl Primitive for MonochromeSprite {}
impl Primitive for SubpixelSprite {}
impl Primitive for PolychromeSprite {}
