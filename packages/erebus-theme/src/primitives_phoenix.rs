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

pub use crate::primitives::{
    AtlasTextureId, AtlasTextureKind, AtlasTile, Background, BackgroundTag, BorderStyle, Bounds,
    ColorSpace, ContentMask, Corners, DeviceBounds, DevicePoint, DeviceSize, Edges, Hsla,
    LinearColorStop, PaddedBool32, Point, Primitive, Rgba, Size, TileId, TransformationMatrix,
};

pub const LAYER_INDEX_MASK: u32 = 0x7fff;
pub const LAYER_MATRIX_BIT: u32 = 0x8000;
pub const LAYER_CLIP_SHIFT: u32 = 16;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PolychromeSprite {
    pub order: u32,
    /// replaces padding
    pub layer: u32,
    pub tile: AtlasTile,
    pub grayscale: PaddedBool32,
    pub opacity: f32,
    pub bounds: Bounds,
    pub content_mask: ContentMask,
    pub corner_radii: Corners,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MonochromeSprite {
    pub order: u32,
    /// replaces padding
    pub layer: u32,
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
    /// 0 = drop shadow, 1 = inset shadow.
    pub inset: u32,
    /// replaces padding
    pub layer: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Underline {
    pub order: u32,
    /// replaces padding
    pub layer: u32,
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
    /// `pad` holds the packed transform/clip word at absolute byte offset 108.
    /// 
    /// Use [`Self::set_background`] to replace the fill while preserving this word.
    pub background: Background,
    pub border_color: Hsla,
    pub corner_radii: Corners,
    pub border_widths: Edges,
}

impl Quad {
    pub const fn layer(&self) -> u32 {
        self.background.pad
    }

    pub fn set_layer(&mut self, layer: u32) {
        self.background.pad = layer;
    }

    /// Replaces the fill without resetting the quad's transform or rounded clip.
    pub fn set_background(&mut self, mut background: Background) {
        background.pad = self.layer();
        self.background = background;
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PathVertex {
    pub xy_position: Point,
    pub st_position: Point,
    pub content_mask: ContentMask,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PathRasterizationVertex {
    pub xy_position: Point,
    pub st_position: Point,
    pub color: Background,
    pub bounds: Bounds,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PathSprite {
    pub bounds: Bounds,
}

/// The 48-byte surface upload record, not the CPU-side `PaintSurface` object.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SurfaceBounds {
    pub bounds: Bounds,
    pub content_mask: ContentMask,
    pub opacity: f32,
    /// Rounded clip index. zero means no clip
    pub clip: u32,
    /// Clockwise quarter turns of the sampled picture.
    pub quarter_turns: u32,
    /// 0 or 1.
    pub mirrored: u32,
}

pub const SCENE_LAYERS_BUFFER_INDEX: u32 = 9;
pub const SCENE_CLIPS_BUFFER_INDEX: u32 = 10;
pub const TARGET_PARAMS_BUFFER_INDEX: u32 = 11;

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneLayer {
    pub values: [f32; 4],
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneClip {
    pub bounds: Bounds,
    pub corner_radii: Corners,
    /// width==zero means no enclosing rrect
    pub enclosing_bounds: Bounds,
    pub enclosing_corner_radii: Corners,
}

#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TargetParams {
    pub viewport_origin: [f32; 2],
    pub path_origin: [f32; 2],
    pub path_size: [f32; 2],
    pub pad: [f32; 2],
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HdrSurfaceParams {
    pub uv: [f32; 4],
    pub src_peak: f32,
    pub dst_peak: f32,
    pub grayscale: PaddedBool32,
    pub pad: u32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EffectUniform {
    pub bounds: [f32; 4],
    pub content_mask: [f32; 4],
    pub rect: [f32; 4],
    pub radii: [f32; 4],
    pub uv: [f32; 4],
    pub filter: [f32; 4],
    pub flags: [u32; 4],
    pub mask_geometry: [f32; 4],
    pub mask_offsets: [[f32; 4]; 2],
    pub mask_alphas: [[f32; 4]; 2],
}

#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BlurUniform {
    pub source_size: [f32; 2],
    pub direction: [f32; 2],
    pub sigma: f32,
    pub radius: f32,
    pub texture_size: [f32; 2],
    pub target_size: [f32; 2],
    pub mirror: f32,
    pub pad: f32,
}

impl Primitive for Quad {}
impl Primitive for Shadow {}
impl Primitive for Underline {}
impl Primitive for MonochromeSprite {}
impl Primitive for PolychromeSprite {}
impl Primitive for PathRasterizationVertex {}
impl Primitive for PathSprite {}
impl Primitive for SurfaceBounds {}
