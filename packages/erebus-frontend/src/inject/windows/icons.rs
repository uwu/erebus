//! Windows icons can contain several sizes and image formats. We extract the largest
//! PNG available so the frontend can display it without decoding Windows bitmaps.

use std::{io, path::Path};

use pelite::{FileMap, PeFile, resources::Name};

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// The largest PNG in icon number `index` of `path`, or the icon with the resource ID
/// `-index` if it's negative.
pub fn png(path: &Path, index: i32) -> io::Result<Option<Vec<u8>>> {
    let map = FileMap::open(path)?;

    Ok(match map.as_ref() {
        bytes @ [b'M', b'Z', ..] => from_executable(bytes, index),
        bytes @ [0, 0, 1, 0, ..] => from_ico(bytes),
        _ => None,
    })
}

fn largest_png<'a>(images: impl IntoIterator<Item = &'a [u8]>) -> Option<Vec<u8>> {
    images
        .into_iter()
        .filter(|image| image.starts_with(PNG_SIGNATURE))
        .max_by_key(|image| {
            let width = image.get(16..20)?.try_into().ok()?;
            Some(u32::from_be_bytes(width))
        })
        .map(<[u8]>::to_vec)
}

fn from_executable(bytes: &[u8], index: i32) -> Option<Vec<u8>> {
    let resources = PeFile::from_bytes(bytes).ok()?.resources().ok()?;
    let groups: Vec<_> = resources.icons().filter_map(Result::ok).collect();

    let group = match index {
        ..0 => groups
            .iter()
            .find(|(name, _)| *name == Name::Id(index.unsigned_abs())),
        _ => groups.get(index as usize),
    }
    .or(groups.first());
    let (_, group) = group?;

    largest_png(
        group
            .entries()
            .iter()
            .filter_map(|entry| group.image(entry.nId).ok()),
    )
}

fn from_ico(bytes: &[u8]) -> Option<Vec<u8>> {
    let u32_at = |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    let count = u16::from_le_bytes(bytes.get(4..6)?.try_into().ok()?) as usize;

    largest_png((0..count).filter_map(|entry| {
        let entry = 6 + entry * 16;
        let size = u32_at(entry + 8)? as usize;
        let offset = u32_at(entry + 12)? as usize;
        bytes.get(offset..offset + size)
    }))
}
