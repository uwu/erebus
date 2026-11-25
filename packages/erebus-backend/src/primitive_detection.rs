//! Primitive detection & classification

use erebus_theme::primitives::{
    AtlasTextureKind, AtlasTile, Background, BackgroundTag, BorderStyle, Bounds, ColorSpace,
    ContentMask, Corners, Edges, Hsla, MonochromeSprite, PaddedBool32, PolychromeSprite, Quad,
    Shadow, SubpixelSprite, TransformationMatrix, Underline,
};

/// Scene primitives that can be reinterpreted from raw bytes in a mapped VkBuffer. This is
/// (for the most part) any child type of GPUI's [`Primitive`] enum.
///
/// [`Primitive`]: https://github.com/zed-industries/zed/blob/main/crates/gpui/src/scene.rs#L222C10-L222C19
pub trait Validate: Copy + PartialEq {
    /// Returns `true` if this instance plausibly came from GPUI rather than being garbage.
    fn looks_valid(&self) -> bool;
}

/// Validates the `pad` field on scene primitives.
///
/// Phoenix shoves renderer metadata in here instead, so we shouldn't assume zero in that case.
fn pad_looks_valid(pad: u32) -> bool {
    cfg!(feature = "phoenix") || pad == 0
}

/// returns true if every value is a finite non-subnormal float within lo..=hi
fn all_in_range(values: &[f32], lo: f32, hi: f32) -> bool {
    values
        .iter()
        .all(|&v| (v == 0.0 || v.is_normal()) && (lo..=hi).contains(&v))
}

impl Validate for Quad {
    // this might be really overkill
    fn looks_valid(&self) -> bool {
        self.border_style.0 <= BorderStyle::DASHED.0
            && self.bounds.looks_valid()
            && self.content_mask.looks_valid()
            && self.background.looks_valid()
            && self.border_color.looks_valid()
            && self.corner_radii.looks_valid()
            && self.border_widths.looks_valid()
    }
}

impl Validate for Underline {
    fn looks_valid(&self) -> bool {
        pad_looks_valid(self.pad)
            && self.bounds.looks_valid()
            && self.content_mask.looks_valid()
            && self.color.looks_valid()
            && all_in_range(&[self.thickness], 0.0, 1e6)
            && self.wavy.looks_valid()
    }
}

impl Validate for Shadow {
    fn looks_valid(&self) -> bool {
        all_in_range(&[self.blur_radius], 0.0, 1e6)
            && self.bounds.looks_valid()
            && self.corner_radii.looks_valid()
            && self.content_mask.looks_valid()
            && self.color.looks_valid()
            && self.element_bounds.looks_valid()
            && self.element_corner_radii.looks_valid()
            && self.inset <= 1
            && pad_looks_valid(self.pad)
    }
}

impl Validate for MonochromeSprite {
    fn looks_valid(&self) -> bool {
        pad_looks_valid(self.pad)
            && self.tile.looks_valid(AtlasTextureKind::MONOCHROME)
            && self.bounds.looks_valid()
            && self.content_mask.looks_valid()
            && self.color.looks_valid()
            && self.transformation.looks_valid()
    }
}

impl Validate for SubpixelSprite {
    fn looks_valid(&self) -> bool {
        pad_looks_valid(self.pad)
            && self.tile.looks_valid(AtlasTextureKind::SUBPIXEL)
            && self.bounds.looks_valid()
            && self.content_mask.looks_valid()
            && self.color.looks_valid()
            && self.transformation.looks_valid()
    }
}

impl Validate for PaddedBool32 {
    fn looks_valid(&self) -> bool {
        self.0 <= 1
    }
}

impl Validate for PolychromeSprite {
    fn looks_valid(&self) -> bool {
        pad_looks_valid(self.pad)
            && self.tile.looks_valid(AtlasTextureKind::POLYCHROME)
            && self.grayscale.looks_valid()
            && all_in_range(&[self.opacity], 0.0, 1.0)
            && self.bounds.looks_valid()
            && self.content_mask.looks_valid()
            && self.corner_radii.looks_valid()
    }
}

impl Validate for Bounds {
    fn looks_valid(&self) -> bool {
        all_in_range(&[self.origin.x, self.origin.y], -1e6, 1e6)
            && all_in_range(&[self.size.width, self.size.height], 0.0, 1e6)
    }
}

impl Validate for ContentMask {
    fn looks_valid(&self) -> bool {
        let b = &self.bounds;
        all_in_range(
            &[b.origin.x, b.origin.y, b.size.width, b.size.height],
            -1e6,
            1e6,
        )
    }
}

impl Validate for Corners {
    fn looks_valid(&self) -> bool {
        all_in_range(
            &[
                self.top_left,
                self.top_right,
                self.bottom_right,
                self.bottom_left,
            ],
            0.0,
            1e6,
        )
    }
}

impl Validate for Hsla {
    fn looks_valid(&self) -> bool {
        all_in_range(&[self.h, self.s, self.l, self.a], 0.0, 1.0)
    }
}

impl Validate for Edges {
    fn looks_valid(&self) -> bool {
        all_in_range(&[self.top, self.right, self.bottom, self.left], 0.0, 1e6)
    }
}

impl Validate for Background {
    fn looks_valid(&self) -> bool {
        self.tag.0 <= BackgroundTag::CHECKERBOARD.0
            && self.color_space.0 <= ColorSpace::OKLAB.0
            && self.solid.looks_valid()
            && pad_looks_valid(self.pad)
    }
}

impl Validate for TransformationMatrix {
    fn looks_valid(&self) -> bool {
        let [[a, b], [c, d]] = self.rotation_scale;
        let [x, y] = self.translation;
        all_in_range(&[a, b, c, d, x, y], -1e6, 1e6)
    }
}

trait ValidateAtlasTexture {
    fn looks_valid(&self, kind: AtlasTextureKind) -> bool;
}
impl ValidateAtlasTexture for AtlasTile {
    fn looks_valid(&self, kind: AtlasTextureKind) -> bool {
        // anything larger than this isn't realistic
        const MAX_DIMENSION: i32 = 1 << 16;

        let (o, s) = (self.bounds.origin, self.bounds.size);
        self.texture_id.kind == kind
            && [o.x, o.y, s.width, s.height]
                .iter()
                .all(|v| (0..=MAX_DIMENSION).contains(v))
    }
}

/// The primitive types we currently scan for and theme in a GPU upload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveKind {
    Shadow,
    Quad,
    Underline,
    MonochromeSprite,
    SubpixelSprite,
    PolychromeSprite,
}

impl PrimitiveKind {
    /// Tries to deduce which primitive type a given upload contains.
    pub fn classify_bytes(bytes: &[u8]) -> Option<Self> {
        let candidates = [
            (PrimitiveKind::Quad, is_array_of::<Quad>(bytes)),
            (PrimitiveKind::Shadow, is_array_of::<Shadow>(bytes)),
            (PrimitiveKind::Underline, is_array_of::<Underline>(bytes)),
            (
                PrimitiveKind::MonochromeSprite,
                is_array_of::<MonochromeSprite>(bytes),
            ),
            (
                PrimitiveKind::SubpixelSprite,
                is_array_of::<SubpixelSprite>(bytes),
            ),
            (
                PrimitiveKind::PolychromeSprite,
                is_array_of::<PolychromeSprite>(bytes),
            ),
        ];

        // only consider canditates that are valid arrays of the primitive
        let mut matches = candidates.iter().filter(|(_, is_valid)| *is_valid);

        match (matches.next(), matches.next()) {
            (Some(&(kind, _)), None) => Some(kind),

            // either the bytes weren't valid for any primitive, or the bytes are
            // possibly valid for more than one different primitive array types. In
            // the second case, that's pretty bad and shouldn't be guessed on, so
            // return None
            _ => None,
        }
    }
}

/// Is `bytes` a non-empty, properly aligned array of `T` where every element looks valid?
fn is_array_of<T: Validate>(bytes: &[u8]) -> bool {
    // SAFETY: `Instance` guarantees every bit pattern is a valid `T`.
    let (head, items, tail) = unsafe { bytes.align_to::<T>() };
    head.is_empty() && tail.is_empty() && !items.is_empty() && items.iter().all(T::looks_valid)
}

/// Phoenix keeps renderer metadata in the padding of each primitive, which themes mustn't
/// touch, so it's put back after `apply`.
#[cfg(feature = "phoenix")]
pub fn preserve_renderer_metadata(
    bytes: &mut [u8],
    kind: PrimitiveKind,
    apply: impl FnOnce(&mut [u8]),
) {
    use std::mem::offset_of;

    let (stride, offset) = match kind {
        PrimitiveKind::Quad => (
            size_of::<Quad>(),
            offset_of!(Quad, background) + offset_of!(Background, pad),
        ),
        PrimitiveKind::Shadow => (size_of::<Shadow>(), offset_of!(Shadow, pad)),
        PrimitiveKind::Underline => (size_of::<Underline>(), offset_of!(Underline, pad)),
        PrimitiveKind::MonochromeSprite => (
            size_of::<MonochromeSprite>(),
            offset_of!(MonochromeSprite, pad),
        ),
        PrimitiveKind::SubpixelSprite => {
            (size_of::<SubpixelSprite>(), offset_of!(SubpixelSprite, pad))
        }
        PrimitiveKind::PolychromeSprite => (
            size_of::<PolychromeSprite>(),
            offset_of!(PolychromeSprite, pad),
        ),
    };
    let metadata: Vec<[u8; 4]> = bytes
        .chunks_exact(stride)
        .map(|item| item[offset..offset + 4].try_into().unwrap())
        .collect();
    apply(bytes);
    for (item, original) in bytes.chunks_exact_mut(stride).zip(metadata) {
        item[offset..offset + 4].copy_from_slice(&original);
    }
}

#[cfg(not(feature = "phoenix"))]
pub fn preserve_renderer_metadata(
    bytes: &mut [u8],
    _kind: PrimitiveKind,
    apply: impl FnOnce(&mut [u8]),
) {
    apply(bytes);
}
