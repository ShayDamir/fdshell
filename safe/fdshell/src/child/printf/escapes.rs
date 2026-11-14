//! Backslash escapes in the `printf` format string.

// mutants: -C --lib -C --test -C printf -- -E 'test(/^child::printf::/) | binary(printf)'

use alloc::vec::Vec;

/// Emit the backslash escape starting at `i`; returns the index of the byte
/// after the sequence. Unknown escapes keep the backslash and let the next
/// byte be processed as a literal.
pub(super) fn emit_escape(fmt: &[u8], i: usize, out: &mut Vec<u8>) -> usize {
    emit_escape_max(fmt, i, out, 3)
}

/// The scanner behind both entry points; `max_digits` bounds the octal run
/// (three digits total on both paths, four when a `%b` run has the `\0`
/// prefix).
fn emit_escape_max(fmt: &[u8], i: usize, out: &mut Vec<u8>, max_digits: usize) -> usize {
    match fmt.get(i + 1).copied().and_then(escaped) {
        None if fmt.get(i + 1).is_some_and(|&c| octal_digit(c).is_some()) => {
            emit_octal(fmt, i, out, max_digits)
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

/// Emit `\ddd` (up to `max_digits` octal digits) as one byte; returns the
/// index past the consumed digits.
fn emit_octal(fmt: &[u8], i: usize, out: &mut Vec<u8>, max_digits: usize) -> usize {
    let mut value = 0u32;
    let mut digits = 0;
    let mut j = i + 1;
    while digits < max_digits {
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
    // `checked_sub`, not `c - b'0'`: the caller asks about any byte, and one
    // below `0` would underflow.
    let d = c.checked_sub(b'0')?;
    (d <= 7).then_some(d as u32)
}

/// Expand the backslash escapes in an argument (for `%b`) into `out`, using
/// the format-string rules plus `%b`'s own: a `\0` prefix takes up to three
/// further octal digits, and `\c` stops output — then `emit_arg` returns
/// `true` and the caller drops the rest of the conversions, format and
/// arguments (the padding due for the converted prefix is still emitted).
pub(super) fn emit_arg(bytes: &[u8], out: &mut Vec<u8>) -> bool {
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        if b != b'\\' {
            out.push(b);
            i += 1;
            continue;
        }
        if bytes.get(i + 1) == Some(&b'c') {
            return true;
        }
        let max_digits = if bytes.get(i + 1) == Some(&b'0') {
            4
        } else {
            3
        };
        i = emit_escape_max(bytes, i, out, max_digits);
    }
    false
}
