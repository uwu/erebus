//! Start menu shortcuts are `.lnk` files ([shell links]).
//!
//! [shell links]: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-shllink/

use std::{
    env,
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
};

use lnk::{LinkFlags, ShellLink, extradata::ExtraDataBlock, linkinfo::LinkInfo};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shortcut {
    /// what the shortcut starts, if it points at a file
    pub target: Option<PathBuf>,
    pub arguments: String,
    /// An explicit icon file overrides the target's icon.
    pub icon_location: Option<PathBuf>,
    /// which icon in that file, or its resource ID if it's negative
    pub icon_index: i32,
    /// Windows Installer resolves the target when this shortcut is launched.
    pub advertised: bool,
}

pub fn read(path: &Path) -> Option<Shortcut> {
    // a malformed shortcut can panic in `lnk` rather than return an error
    let link = panic::catch_unwind(AssertUnwindSafe(|| {
        ShellLink::open(path, lnk::encoding::WINDOWS_1252)
    }))
    .ok()?
    .ok()?;
    let flags = *link.header().link_flags();
    let strings = link.string_data();

    let mut env_target = None;
    let mut env_icon = None;
    for block in link.extra_data().blocks() {
        match block {
            ExtraDataBlock::EnvironmentProps(block) => {
                env_target = Some(
                    block
                        .target_unicode()
                        .clone()
                        .unwrap_or_else(|| block.target_ansi().clone()),
                );
            }
            ExtraDataBlock::IconEnvironmentProps(block) => {
                env_icon = Some(
                    block
                        .target_unicode()
                        .clone()
                        .unwrap_or_else(|| block.target_ansi().clone()),
                );
            }
            _ => {}
        }
    }

    let relative = || Some(path.parent()?.join(text(strings.relative_path())?));
    let target = env_target
        .filter(|target| !target.is_empty())
        .map(|target| PathBuf::from(expand(&target)))
        .or_else(|| link.link_info().as_ref().and_then(local_path))
        .or_else(relative);

    Some(Shortcut {
        target,
        arguments: text(strings.command_line_arguments()).unwrap_or_default(),
        icon_location: env_icon
            .or_else(|| text(strings.icon_location()))
            .filter(|location| !location.is_empty())
            .map(|location| PathBuf::from(expand(&location))),
        icon_index: *link.header().icon_index(),
        advertised: flags.contains(LinkFlags::HAS_DARWIN_ID),
    })
}

/// Some shortcuts include the terminating NUL in the string's length.
fn text(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(|value| value.trim_end_matches('\0').to_owned())
}

fn local_path(info: &LinkInfo) -> Option<PathBuf> {
    let base = info
        .local_base_path_unicode()
        .as_deref()
        .or(info.local_base_path())?;
    let suffix = info
        .common_path_suffix_unicode()
        .as_deref()
        .unwrap_or(info.common_path_suffix());
    Some(PathBuf::from(format!("{base}{suffix}")))
}

/// Fills in `%VARIABLE%`s from the environment, leaving ones that aren't set as they are.
pub fn expand(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];

        let Some(end) = after.find('%') else {
            out.push_str(&rest[start..]);
            return out;
        };

        let name = &after[..end];
        match env::var(name) {
            Ok(value) if !name.is_empty() => out.push_str(&value),
            _ => {
                out.push('%');
                out.push_str(name);
                out.push('%');
            }
        }
        rest = &after[end + 1..];
    }

    out.push_str(rest);
    out
}

/// Split on unquoted whitespace, keeping double-quoted arguments together.
pub fn split_arguments(arguments: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current: Option<String> = None;
    let mut quoted = false;

    for char in arguments.chars() {
        match char {
            '"' => {
                quoted = !quoted;
                current.get_or_insert_with(String::new);
            }
            ' ' | '\t' if !quoted => args.extend(current.take()),
            char => current.get_or_insert_with(String::new).push(char),
        }
    }

    args.extend(current);
    args
}
