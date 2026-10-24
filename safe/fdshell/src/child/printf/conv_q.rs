//! `%q`: shell-quoting of an argument — the shortest form that re-reads as the
//! same word: unquoted, backslash-escaped, or the `$'…'` form. The UTF-8 unit
//! the rules are written over is decoded in `decode`.

mod decode;
mod dollar;

use alloc::vec::Vec;
pub(super) use decode::Unit;
use decode::decode;

/// Shell-quote `bytes` (already precision-truncated) for re-use as one word.
pub(super) fn render(bytes: &[u8]) -> Vec<u8> {
    let units = decode(bytes);
    if units.is_empty() {
        return b"''".to_vec();
    }
    match style(&units) {
        Style::Unquoted => bytes.to_vec(),
        Style::Escaped => escape_form(&units),
        Style::Dollar => dollar::dollar_form(&units),
    }
}

/// The quoting form chosen for the whole argument.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(Debug))]
enum Style {
    Unquoted,
    Escaped,
    Dollar,
}

/// The strongest form any unit requires: a control byte, DEL, or invalid UTF-8
/// forces `$'…'`; any escapable character forces backslash-escaping.
fn style(units: &[Unit]) -> Style {
    let mut s = Style::Unquoted;
    for (i, u) in units.iter().enumerate() {
        match u {
            Unit::Byte(_) => s = Style::Dollar,
            Unit::Char(c) if c.is_control() || *c as u32 == 0x7f => s = Style::Dollar,
            Unit::Char(c) if needs_escape(*c, i == 0) && s < Style::Dollar => s = Style::Escaped,
            _ => {}
        }
    }
    s
}

/// A character that is not safe to leave bare (so it needs a backslash, or the
/// argument needs the `$'…'` form). `#`/`~` are unsafe only at the start.
fn needs_escape(c: char, at_start: bool) -> bool {
    if c.is_ascii_alphanumeric() {
        return false;
    }
    if matches!(c, '_' | '-' | '.' | ':' | '/' | '@' | '=' | '%' | '+') {
        return false;
    }
    if matches!(c, '#' | '~') {
        return at_start;
    }
    c.is_ascii()
}

/// Backslash-escape the non-safe characters, leaving the rest (including valid
/// multi-byte UTF-8) raw.
fn escape_form(units: &[Unit]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, u) in units.iter().enumerate() {
        let Unit::Char(c) = u else {
            continue;
        };
        if needs_escape(*c, i == 0) {
            out.push(b'\\');
        }
        out.extend_from_slice(c.encode_utf8(&mut [0u8; 4]).as_bytes());
    }
    out
}
