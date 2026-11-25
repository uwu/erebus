//! Apps load our `d3d11.dll` from beside their executable in place of the system copy
//! (see [`d3d11`]). We find apps through Start menu shortcuts, then copy the backend
//! next to the executable each shortcut points to.
//!
//! This works regardless of how the app is launched. Since we don't control its launch
//! environment, we leave the app ID in a marker file beside the backend.

pub mod d3d11;
pub mod icons;
pub mod installed;
pub mod shortcut;
