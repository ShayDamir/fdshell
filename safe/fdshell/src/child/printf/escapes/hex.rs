//! The `\xhh` arm of printf's escape scanner: one or two hex digits name the
//! byte, and no digit is bash's diagnostic at rc 0 (measured against bash 5.3.9).

// mutants: -C --lib -C --test -C printf -- -E 'test(/^child::printf::/) | binary(printf)'

use alloc::vec::Vec;

/// The digit cap bash applies to `\x` (ISO C's `\xhh` is open-ended, bash stops
/// at two — measured: `\x414` is `A` plus a literal `4`).
const MAX_DIGITS: usize = 2;

/// bash's `\x` diagnostic for no hex digit: once per occurrence, and it must
/// not raise the sink's failure flag (bash stays at rc 0). The `\\x` is the
/// literal backslash bash writes; the `\n` is the line break.
const NO_DIGIT: &[u8] = b"printf: missing hex digit for \\x\n";

/// Emit the `\x` escape at `i` — the byte after the backslash is `x` — and
/// return the index past the digits. `None` means no hex digit: the diagnostic
/// has been made and the caller leaves the escape bytes literal.
pub(super) fn emit(fmt: &[u8], i: usize, out: &mut Vec<u8>, err: &mut Vec<u8>) -> Option<usize> {
    let mut value = 0u32;
    let mut digits = 0;
    while digits < MAX_DIGITS {
        match fmt.get(i + 2 + digits).copied().and_then(hex_digit) {
            Some(d) => {
                value = value * 16 + d;
                digits += 1;
            }
            None => break,
        }
    }
    if digits == 0 {
        err.extend_from_slice(NO_DIGIT);
        return None;
    }
    out.push(value as u8);
    Some(i + 2 + digits)
}

/// One hex digit, the `checked_sub` convention of `escapes.rs` (the caller asks
/// about any byte).
fn hex_digit(c: u8) -> Option<u32> {
    if let Some(d) = c.checked_sub(b'0').filter(|d| *d <= 9) {
        return Some(d as u32);
    }
    if let Some(d) = c.checked_sub(b'a').filter(|d| *d <= 5) {
        return Some(u32::from(d) + 10);
    }
    if let Some(d) = c.checked_sub(b'A').filter(|d| *d <= 5) {
        return Some(u32::from(d) + 10);
    }
    None
}
