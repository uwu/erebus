//! The `erebus.manifest` custom section lets a host inspect a theme without running it.
//!
//! Version 1 starts with two little-endian u32s: the format version and theme ABI version.
//!
//! Strings have a u32 byte length followed by UTF-8. Lists start with a u32 item count.
//! Integers and floats use fixed-width little-endian bytes, never native struct layout.
//! The payload follows `Manifest` field order; versions contain three u64 components.
//! Const manifests use `Version::new`, since semver's prerelease/build constructors aren't const.
//! Setting tags are 0=bool, 1=int, 2=float, 3=color, 4=choice, followed by that variant's
//! fields in declaration order. Colors contain four f32s in RGBA order.

use std::io;

use crate::{
    Manifest, OwnedManifest, Version,
    primitives::Rgba,
    settings::{OwnedChoiceOption, OwnedSettingKind, OwnedSettingsField, SettingKind},
};

pub const SECTION_NAME: &str = "erebus.manifest";
pub const FORMAT_VERSION: u32 = 1;
pub const MAX_SIZE: usize = 1024 * 1024;

struct Writer<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> Writer<N> {
    const fn byte(&mut self, value: u8) {
        assert!(self.len < MAX_SIZE, "theme manifest exceeds 1 MiB");
        // The zero-sized writer measures the payload before we allocate the static array.
        if N != 0 {
            self.bytes[self.len] = value;
        }
        self.len += 1;
    }

    const fn bytes(&mut self, bytes: &[u8]) {
        let mut i = 0;
        while i < bytes.len() {
            self.byte(bytes[i]);
            i += 1;
        }
    }

    const fn count(&mut self, count: usize) {
        assert!(count <= u32::MAX as usize, "manifest field is too large");
        self.bytes(&(count as u32).to_le_bytes());
    }

    const fn string(&mut self, value: &str) {
        self.count(value.len());
        self.bytes(value.as_bytes());
    }

    const fn manifest(mut self, manifest: &Manifest) -> Self {
        self.bytes(&FORMAT_VERSION.to_le_bytes());
        self.bytes(&crate::abi::ABI_VERSION.to_le_bytes());
        self.string(manifest.name);
        self.string(manifest.author);
        self.string(manifest.description);
        self.bytes(&manifest.version.major.to_le_bytes());
        self.bytes(&manifest.version.minor.to_le_bytes());
        self.bytes(&manifest.version.patch.to_le_bytes());
        self.count(manifest.settings.len());
        let mut i = 0;
        while i < manifest.settings.len() {
            let field = &manifest.settings[i];
            self.string(field.key);
            self.string(field.label);
            self.string(field.description);
            match field.kind {
                SettingKind::Bool { default } => {
                    self.byte(0);
                    self.byte(default as u8);
                }
                SettingKind::Int { default, min, max } => {
                    self.byte(1);
                    self.bytes(&default.to_le_bytes());
                    self.bytes(&min.to_le_bytes());
                    self.bytes(&max.to_le_bytes());
                }
                SettingKind::Float {
                    default,
                    min,
                    max,
                    step,
                } => {
                    self.byte(2);
                    self.bytes(&default.to_le_bytes());
                    self.bytes(&min.to_le_bytes());
                    self.bytes(&max.to_le_bytes());
                    self.bytes(&step.to_le_bytes());
                }
                SettingKind::Color { default } => {
                    self.byte(3);
                    self.bytes(&default.r.to_le_bytes());
                    self.bytes(&default.g.to_le_bytes());
                    self.bytes(&default.b.to_le_bytes());
                    self.bytes(&default.a.to_le_bytes());
                }
                SettingKind::Choice { default, options } => {
                    self.byte(4);
                    self.string(default);
                    self.count(options.len());
                    let mut j = 0;
                    while j < options.len() {
                        self.string(options[j].value);
                        self.string(options[j].label);
                        j += 1;
                    }
                }
            }
            i += 1;
        }
        self
    }
}

#[doc(hidden)]
pub const fn encoded_len(manifest: &Manifest) -> usize {
    Writer { bytes: [], len: 0 }.manifest(manifest).len
}

#[doc(hidden)]
pub const fn encode<const N: usize>(manifest: &Manifest) -> [u8; N] {
    let writer = Writer {
        bytes: [0; N],
        len: 0,
    }
    .manifest(manifest);
    assert!(writer.len == N, "incorrect manifest buffer size");
    writer.bytes
}

fn invalid(message: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn bytes<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let (value, rest) = self
            .0
            .split_at_checked(N)
            .ok_or_else(|| invalid("truncated theme manifest"))?;
        self.0 = rest;
        Ok(value.try_into().unwrap())
    }

    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.bytes()?))
    }

    fn string(&mut self) -> io::Result<String> {
        let len = self.u32()? as usize;
        let (value, rest) = self
            .0
            .split_at_checked(len)
            .ok_or_else(|| invalid("truncated manifest string"))?;
        self.0 = rest;
        Ok(std::str::from_utf8(value).map_err(invalid)?.to_owned())
    }

    fn count(&mut self, minimum_size: usize) -> io::Result<usize> {
        let count = self.u32()? as usize;
        if count > self.0.len() / minimum_size {
            return Err(invalid("manifest list exceeds the remaining data"));
        }
        Ok(count)
    }

    fn kind(&mut self) -> io::Result<OwnedSettingKind> {
        Ok(match self.bytes::<1>()?[0] {
            0 => OwnedSettingKind::Bool {
                default: match self.bytes::<1>()?[0] {
                    0 => false,
                    1 => true,
                    _ => return Err(invalid("invalid boolean in theme manifest")),
                },
            },
            1 => OwnedSettingKind::Int {
                default: i64::from_le_bytes(self.bytes()?),
                min: i64::from_le_bytes(self.bytes()?),
                max: i64::from_le_bytes(self.bytes()?),
            },
            2 => OwnedSettingKind::Float {
                default: f64::from_le_bytes(self.bytes()?),
                min: f64::from_le_bytes(self.bytes()?),
                max: f64::from_le_bytes(self.bytes()?),
                step: f64::from_le_bytes(self.bytes()?),
            },
            3 => OwnedSettingKind::Color {
                default: Rgba {
                    r: f32::from_le_bytes(self.bytes()?),
                    g: f32::from_le_bytes(self.bytes()?),
                    b: f32::from_le_bytes(self.bytes()?),
                    a: f32::from_le_bytes(self.bytes()?),
                },
            },
            4 => {
                let default = self.string()?;
                let count = self.count(8)?;
                let mut options = Vec::with_capacity(count);
                for _ in 0..count {
                    options.push(OwnedChoiceOption {
                        value: self.string()?,
                        label: self.string()?,
                    });
                }
                OwnedSettingKind::Choice { default, options }
            }
            _ => return Err(invalid("unknown setting kind in theme manifest")),
        })
    }
}

pub fn decode(bytes: &[u8]) -> io::Result<OwnedManifest> {
    if bytes.len() > MAX_SIZE {
        return Err(invalid("theme manifest exceeds 1 MiB"));
    }
    let mut reader = Reader(bytes);
    let format = reader.u32()?;
    if format != FORMAT_VERSION {
        return Err(invalid(format!(
            "unsupported theme metadata version {format}"
        )));
    }
    let abi = reader.u32()?;
    if abi != crate::abi::ABI_VERSION {
        return Err(invalid(format!(
            "theme requires ABI {abi}, but this frontend supports ABI {}",
            crate::abi::ABI_VERSION
        )));
    }
    let name = reader.string()?;
    let author = reader.string()?;
    let description = reader.string()?;
    let version = Version::new(
        u64::from_le_bytes(reader.bytes()?),
        u64::from_le_bytes(reader.bytes()?),
        u64::from_le_bytes(reader.bytes()?),
    );
    let count = reader.count(14)?;
    let mut settings = Vec::with_capacity(count);
    for _ in 0..count {
        settings.push(OwnedSettingsField {
            key: reader.string()?,
            label: reader.string()?,
            description: reader.string()?,
            kind: reader.kind()?,
        });
    }
    if !reader.0.is_empty() {
        return Err(invalid("unexpected trailing data in theme manifest"));
    }
    validate(&settings)?;
    Ok(OwnedManifest {
        name,
        author,
        description,
        version,
        settings,
    })
}

fn validate(fields: &[OwnedSettingsField]) -> io::Result<()> {
    use std::collections::HashSet;
    let mut keys = HashSet::new();
    for field in fields {
        if field.key.is_empty() || !keys.insert(&field.key) {
            return Err(invalid("empty or duplicate setting key"));
        }
        let valid = match &field.kind {
            OwnedSettingKind::Bool { .. } => true,
            OwnedSettingKind::Int { default, min, max } => min <= default && default <= max,
            OwnedSettingKind::Float {
                default,
                min,
                max,
                step,
            } => {
                [default, min, max, step].iter().all(|v| v.is_finite())
                    && min <= default
                    && default <= max
                    && *step >= 0.0
            }
            OwnedSettingKind::Color { default } => [default.r, default.g, default.b, default.a]
                .iter()
                .all(|v| (0.0..=1.0).contains(v)),
            OwnedSettingKind::Choice { default, options } => {
                let mut values = HashSet::new();
                options.iter().any(|option| &option.value == default)
                    && options.iter().all(|option| values.insert(&option.value))
            }
        };
        if !valid {
            return Err(invalid(format!(
                "invalid schema for setting {:?}",
                field.key
            )));
        }
    }
    Ok(())
}
