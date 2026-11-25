//! The Linux backend is a Vulkan layer that stays disabled until
//! [`ENABLE_ENV`](vulkan::ENABLE_ENV) is set. For ordinary apps we add that variable and
//! the app ID to the desktop entry's `Exec` lines (see [`installed`]). Flatpak manages
//! its own launch environment, so those apps need an override instead (see [`flatpak`]).

pub mod desktop_entry;
pub mod flatpak;
pub mod installed;
pub mod vulkan;
