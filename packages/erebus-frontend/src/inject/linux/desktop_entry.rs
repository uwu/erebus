//! Only rewrite the lines we're changing in a [desktop entry]. Re-serializing the whole
//! file would lose comments, ordering and translations. Flatpak overrides use the same
//! format, so they go through this editor too.
//!
//! [desktop entry]: https://specifications.freedesktop.org/desktop-entry-spec/latest/

use std::ops::Range;

pub const MAIN_GROUP: &str = "Desktop Entry";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    lines: Vec<String>,
}

fn group_header(line: &str) -> Option<&str> {
    line.trim()
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
}

fn key_value(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let (key, value) = trimmed.split_once('=')?;
    Some((key.trim_end(), value.trim_start()))
}

impl DesktopEntry {
    pub fn parse(text: &str) -> Self {
        Self {
            lines: text.split('\n').map(str::to_owned).collect(),
        }
    }

    /// Each group's name and the range of lines after its header.
    fn groups(&self) -> Vec<(&str, Range<usize>)> {
        let mut groups: Vec<(&str, Range<usize>)> = Vec::new();

        for (index, line) in self.lines.iter().enumerate() {
            if let Some(name) = group_header(line) {
                if let Some((_, range)) = groups.last_mut() {
                    range.end = index;
                }
                groups.push((name, index + 1..self.lines.len()));
            }
        }

        groups
    }

    fn group(&self, group: &str) -> Option<Range<usize>> {
        self.groups()
            .into_iter()
            .find(|(name, _)| *name == group)
            .map(|(_, range)| range)
    }

    /// Preserves the order of groups in the file.
    pub fn group_names(&self) -> Vec<String> {
        self.groups()
            .into_iter()
            .map(|(name, _)| name.to_owned())
            .collect()
    }

    fn find(&self, group: &str, key: &str) -> Option<usize> {
        self.group(group)?
            .find(|&index| key_value(&self.lines[index]).is_some_and(|(k, _)| k == key))
    }

    /// The value of `key` as written in the file, escapes and all.
    pub fn raw(&self, group: &str, key: &str) -> Option<&str> {
        let index = self.find(group, key)?;
        key_value(&self.lines[index]).map(|(_, value)| value)
    }

    /// The value of a string key with its escapes resolved.
    pub fn string(&self, group: &str, key: &str) -> Option<String> {
        self.raw(group, key).map(unescape)
    }

    pub fn bool(&self, group: &str, key: &str) -> bool {
        self.raw(group, key).is_some_and(|value| value == "true")
    }

    /// Try `locales` in order, then fall back to the unlocalized key.
    pub fn localized(&self, group: &str, key: &str, locales: &[String]) -> Option<String> {
        locales
            .iter()
            .find_map(|locale| self.string(group, &format!("{key}[{locale}]")))
            .or_else(|| self.string(group, key))
    }

    /// Pass an already escaped value. New keys go after the group's last key; a missing group
    /// is left alone.
    pub fn set_raw(&mut self, group: &str, key: &str, raw: &str) {
        let line = format!("{key}={raw}");

        if let Some(index) = self.find(group, key) {
            self.lines[index] = line;
            return;
        }

        let Some(range) = self.group(group) else {
            return;
        };
        // keep blank lines and comments between groups where they are
        let at = range
            .clone()
            .rev()
            .find(|&index| key_value(&self.lines[index]).is_some())
            .map_or(range.start, |index| index + 1);
        self.lines.insert(at, line);
    }

    /// Adds an empty group at the end if there's none called `group`.
    pub fn ensure_group(&mut self, group: &str) {
        if self.group(group).is_some() {
            return;
        }

        // Without a trailing newline, we need to finish the last line before adding a header.
        if self.lines.last().is_some_and(|line| !line.is_empty()) {
            self.lines.push(String::new());
        }
        if self.lines.iter().any(|line| !line.trim().is_empty()) {
            // a blank line between groups, keeping the one the file ends with
            self.lines.insert(self.lines.len() - 1, String::new());
        }
        let at = self.lines.len().saturating_sub(1);
        self.lines.insert(at, format!("[{group}]"));
    }

    /// Removes `group` if it has no keys, along with the blank lines before it.
    pub fn remove_if_empty(&mut self, group: &str) {
        let Some(range) = self.group(group) else {
            return;
        };
        if range
            .clone()
            .any(|index| key_value(&self.lines[index]).is_some())
        {
            return;
        }

        let header = range.start - 1;
        let (start, end) = if range.end < self.lines.len() {
            // Keep the blank lines before the next group's header.
            (header, range.end)
        } else {
            // For the last group, remove the preceding blank lines but preserve the final
            // newline.
            let mut start = header;
            while start > 0 && self.lines[start - 1].trim().is_empty() {
                start -= 1;
            }
            (start, self.lines.len() - 1)
        };
        self.lines.drain(start..end.max(start));
    }

    /// Whether any group has a key.
    pub fn has_keys(&self) -> bool {
        self.lines.iter().any(|line| key_value(line).is_some())
    }

    pub fn set_string(&mut self, group: &str, key: &str, value: &str) {
        self.set_raw(group, key, &escape(value));
    }

    pub fn remove(&mut self, group: &str, key: &str) {
        if let Some(index) = self.find(group, key) {
            self.lines.remove(index);
        }
    }
}

impl std::fmt::Display for DesktopEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.lines.join("\n"))
    }
}

/// Resolves the escapes every string value can have.
pub fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();

    while let Some(char) = chars.next() {
        if char != '\\' {
            out.push(char);
            continue;
        }

        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            // not an escape the spec knows, so leave it for whoever reads the value next
            // (`Exec` has escapes of its own)
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }

    out
}

pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for char in value.chars() {
        match char {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            char => out.push(char),
        }
    }
    out
}

/// Quote a single argument for `Exec`, then escape it for storage in the desktop entry.
pub fn quote_exec_arg(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || "-_./=:+,@".contains(char));
    if plain {
        return arg.to_owned();
    }

    let mut quoted = String::with_capacity(arg.len() + 2);
    quoted.push('"');
    for char in arg.chars() {
        match char {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(char);
            }
            // a literal % is written %% even in quotes, so it isn't read as a field code
            '%' => quoted.push_str("%%"),
            char => quoted.push(char),
        }
    }
    quoted.push('"');
    escape(&quoted)
}

/// Splits an (unescaped) `Exec` value into arguments, following the spec's quoting rules.
/// Field codes like `%U` are left in place.
pub fn split_exec(exec: &str) -> Option<Vec<String>> {
    let mut args = Vec::new();
    let mut current: Option<String> = None;
    let mut chars = exec.chars();

    while let Some(char) = chars.next() {
        match char {
            ' ' | '\t' => {
                if let Some(arg) = current.take() {
                    args.push(arg);
                }
            }
            '"' => {
                let arg = current.get_or_insert_with(String::new);
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' => match chars.next()? {
                            escaped @ ('"' | '`' | '$' | '\\') => arg.push(escaped),
                            other => {
                                arg.push('\\');
                                arg.push(other);
                            }
                        },
                        other => arg.push(other),
                    }
                }
            }
            other => current.get_or_insert_with(String::new).push(other),
        }
    }

    args.extend(current);
    Some(args)
}

/// Skip any `env VAR=value ...` prefix to find the executable.
pub fn exec_program(exec: &str) -> Option<String> {
    let args = split_exec(exec)?;
    let mut args = args.into_iter();
    let first = args.next()?;

    if first.rsplit('/').next() != Some("env") {
        return Some(first);
    }

    args.find(|arg| !arg.contains('=') && !arg.starts_with('-'))
}
