//! Everything related to theme loading and execution by backends.

pub mod engine;
pub mod ipc;
pub(crate) mod primitive_detection;
pub mod themes;

pub use engine::{ThemeCtx, ThemeModule};
pub use primitive_detection::{PrimitiveKind, preserve_renderer_metadata};
pub use wasmtime::Result;
