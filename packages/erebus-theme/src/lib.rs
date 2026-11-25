#[doc(hidden)]
pub mod abi;
pub mod primitives;

use primitives::{MonochromeSprite, PolychromeSprite, Quad, Shadow, SubpixelSprite, Underline};
pub mod settings;

pub use semver::Version;

use crate::settings::{OwnedSettingsField, Settings, SettingsField};

pub trait Theme: Sized + 'static {
    /// The theme's metadata and settings.
    const MANIFEST: Manifest;

    //
    // lifecycle
    //

    fn init(settings: &Settings) -> Self;

    //
    // theme methods
    //

    fn quad(&mut self, _quad: &mut Quad) {}
    fn shadow(&mut self, _shadow: &mut Shadow) {}
    fn underline(&mut self, _underline: &mut Underline) {}
    fn monochrome_sprite(&mut self, _sprite: &mut MonochromeSprite) {}
    fn subpixel_sprite(&mut self, _sprite: &mut SubpixelSprite) {}
    fn polychrome_sprite(&mut self, _sprite: &mut PolychromeSprite) {}
}

#[derive(Clone, PartialEq, serde::Serialize)]
pub struct Manifest {
    pub name: &'static str,
    pub author: &'static str,
    pub description: &'static str,
    pub version: Version,
    pub settings: &'static [SettingsField],
}

/// Owned version of [`Manifest`] that can be deserialized.
///
/// [`Manifest`] must be constructable in const-contexts which means we're forced
/// to use static slices and whatnot rather than Vec/String. This version of the type
/// is one with owned variants of those fields so it can support serde::Deserialize
/// to decode the manifest stored in a compiled theme.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct OwnedManifest {
    pub name: String,
    pub author: String,
    pub description: String,
    pub version: Version,
    pub settings: Vec<OwnedSettingsField>,
}

#[macro_export]
macro_rules! export_theme {
    (@primitives $theme:ty { $($export:ident => $method:ident($primitive:ident)),* $(,)? }) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "C" fn $export(ptr: u32, len: u32) -> u32 {
                $crate::abi::helpers::with_theme(&__EREBUS_THEME, |theme: &mut $theme| {
                    unsafe {
                        $crate::abi::helpers::apply::<$crate::primitives::$primitive>(ptr, len, |item| {
                            $crate::Theme::$method(theme, item)
                        }) as u32
                    }
                })
            }
        )*
    };

    ($theme:ty) => {
        const _: () = {
            const __MANIFEST: &'static $crate::Manifest = &<$theme as $crate::Theme>::MANIFEST;
            const _: () = $crate::settings::validate_settings(__MANIFEST.settings);

            #[cfg(target_arch = "wasm32")]
            #[used]
            #[unsafe(link_section = "erebus.manifest")]
            static __EREBUS_MANIFEST: [u8; $crate::abi::metadata::encoded_len(__MANIFEST)] =
                $crate::abi::metadata::encode(__MANIFEST);

            ::std::thread_local! {
                static __EREBUS_THEME: ::core::cell::RefCell<::core::option::Option<$theme>> =
                    const { ::core::cell::RefCell::new(::core::option::Option::None) };
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn init(ptr: u32, len: u32) {
                unsafe {
                    $crate::abi::helpers::init(&__EREBUS_THEME, ptr, len);
                }
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn abi_version() -> u32 {
                $crate::abi::ABI_VERSION
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn alloc(len: u32) -> u32 {
                $crate::abi::helpers::alloc(len)
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn dealloc(ptr: u32, len: u32) {
                unsafe { $crate::abi::helpers::dealloc(ptr, len) }
            }

            $crate::export_theme!(@primitives $theme {
                theme_quad => quad(Quad),
                theme_shadow => shadow(Shadow),
                theme_underline => underline(Underline),
                theme_monochrome_sprite => monochrome_sprite(MonochromeSprite),
                theme_polychrome_sprite => polychrome_sprite(PolychromeSprite),
                theme_subpixel_sprite => subpixel_sprite(SubpixelSprite),
            });
        };
    };
}
