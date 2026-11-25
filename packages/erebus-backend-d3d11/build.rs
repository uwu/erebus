//! erebus-backend-d3d11 uses DLL hijacking to get the app to load our own
//! d3d11.dll instead of the one in C:/Windows/System32/d3d11.dll. The problem
//! is that we are now expected to provide all of the procedures in the original
//! DLL through our replaced copy.
//!
//! If we just used the output DLL as is, GPUI would fail to start because
//! it relies on more than just the functions we patch... So, we need to read
//! an d3d11.dll and forward every export *other* than the functions we hook.
//!
//! Also, annoyingly, we can't just forward to the d3d11.dll in System32, since
//! .def files don't support absolute paths. If we just pointed the forwarded
//! functions to LIBRARY d3d11, then we'd be pointing at ourselves because again,
//! we're hijacking the dll. Instead we expect a copy of the original dll to be
//! present in the search path at d3d11_orig.dll. Yeesh, I hate windows.

use std::path::Path;

fn main() {
    let def = Path::new(env!("CARGO_MANIFEST_DIR")).join("d3d11_forwarders.def");
    println!("cargo:rerun-if-changed={}", def.display());
    println!("cargo:rustc-link-arg-cdylib=/DEF:{}", def.display());
}
