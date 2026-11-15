//! The parts of a frontend that don't care which UI toolkit draws them: the theme library on
//! disk, what we know about running apps, and watching theme sources for changes.

pub mod ipc;
pub mod library;
pub mod apps;
pub mod i18n;
pub mod watcher;
