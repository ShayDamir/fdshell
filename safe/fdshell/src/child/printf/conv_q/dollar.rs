//! The `$'…'` quoting form: named/octal escapes for control and special bytes,
//! valid multi-byte UTF-8 kept raw.

use alloc::vec::Vec;

use super::Unit;

/// Render the argument in the `$'…'` form.
pub(super) fn dollar_form(units: &[Unit]) -> Vec<u8> {
    let mut out = b"$'".to_vec();
    for u in units {
        match u {
            Unit::Byte(b) => octal3(&mut out, *b),
            Unit::Char(c) => dollar_char(&mut out, *c),
        }
    }
    out.push(b'\'');
    out
}

fn dollar_char(out: &mut Vec<u8>, c: char) {
    if !c.is_ascii() {
        out.extend_from_slice(c.encode_utf8(&mut [0u8; 4]).as_bytes());
        return;
    }
    let b = c as u8;
    match named_escape(b) {
        Some(e) => out.extend_from_slice(e.as_bytes()),
        None if (0x20..=0x7e).contains(&b) => out.push(b),
        None => octal3(out, b),
    }
}

fn named_escape(b: u8) -> Option<&'static str> {
    match b {
        0x07 => Some("\\a"),
        0x08 => Some("\\b"),
        0x09 => Some("\\t"),
        0x0a => Some("\\n"),
        0x0b => Some("\\v"),
        0x0c => Some("\\f"),
        0x0d => Some("\\r"),
        0x1b => Some("\\E"),
        0x5c => Some("\\\\"),
        0x27 => Some("\\'"),
        _ => None,
    }
}

fn octal3(out: &mut Vec<u8>, b: u8) {
    out.extend_from_slice(alloc::format!("\\{b:03o}").as_bytes());
}
