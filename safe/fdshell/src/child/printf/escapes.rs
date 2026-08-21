//! Backslash escapes in the `printf` format string.

use alloc::vec::Vec;

/// Emit the backslash escape starting at `i`; returns the index of the byte
/// after the sequence. Unknown escapes keep the backslash and let the next
/// byte be processed as a literal.
pub(super) fn emit_escape(fmt: &[u8], i: usize, out: &mut Vec<u8>) -> usize {
    match fmt.get(i + 1).copied().and_then(escaped) {
        None if fmt.get(i + 1).is_some_and(|&c| (b'0'..=b'7').contains(&c)) => {
            emit_octal(fmt, i, out)
        }
        Some(b) => {
            out.push(b);
            i + 2
        }
        None => {
            // Unknown escape (or trailing backslash): keep the backslash and
            // let the next byte be processed as a literal.
            out.push(b'\\');
            i + 1
        }
    }
}

/// The byte a recognized escape maps to.
fn escaped(c: u8) -> Option<u8> {
    match c {
        b'n' => Some(b'\n'),
        b't' => Some(b'\t'),
        b'r' => Some(b'\r'),
        b'a' => Some(0x07),
        b'b' => Some(0x08),
        b'f' => Some(0x0c),
        b'v' => Some(0x0b),
        b'\\' => Some(b'\\'),
        _ => None,
    }
}

/// Emit `\ddd` (up to three octal digits) as one byte; returns the index past
/// the consumed digits.
fn emit_octal(fmt: &[u8], i: usize, out: &mut Vec<u8>) -> usize {
    let mut value = 0u32;
    let mut digits = 0;
    let mut j = i + 1;
    while digits < 3 {
        match fmt.get(j).copied().and_then(octal_digit) {
            Some(d) => {
                value = value * 8 + d;
                digits += 1;
                j += 1;
            }
            None => break,
        }
    }
    out.push(value as u8);
    j
}

fn octal_digit(c: u8) -> Option<u32> {
    (b'0'..=b'7').contains(&c).then_some((c - b'0') as u32)
}
