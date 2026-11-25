use std::io;

use erebus_theme::{OwnedManifest, abi::metadata};
use wasmparser::{Encoding, Parser, Payload};

fn invalid(message: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub(crate) fn read(wasm: &[u8]) -> io::Result<OwnedManifest> {
    let mut manifest = None;
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.map_err(invalid)? {
            Payload::Version {
                encoding: Encoding::Component,
                ..
            } => {
                return Err(invalid("expected a WebAssembly module, not a component"));
            }
            Payload::CustomSection(section) if section.name() == metadata::SECTION_NAME => {
                if manifest.is_some() {
                    return Err(invalid("duplicate erebus.manifest section"));
                }
                manifest = Some(metadata::decode(section.data())?);
            }
            _ => {}
        }
    }
    let manifest = manifest.ok_or_else(|| invalid("theme has no erebus.manifest metadata"))?;
    Ok(manifest)
}
