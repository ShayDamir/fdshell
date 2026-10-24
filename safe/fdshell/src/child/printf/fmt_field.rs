//! Shared field padding and sign split for the `printf` formatters.

use alloc::vec::Vec;

/// Pad `prefix` + `core` to `width` bytes and append to `out`.
///
/// `left` left-justifies (space-fill on the right). `zero` zero-fills the
/// `core` only — never the `prefix` (the sign) — when right-justified;
/// otherwise spaces fill. A `width` at most the combined length prints the
/// field unchanged.
pub(super) fn pad(
    left: bool,
    zero: bool,
    width: usize,
    prefix: &[u8],
    core: &[u8],
    out: &mut Vec<u8>,
) {
    let fill = width.saturating_sub(prefix.len() + core.len());
    if left {
        out.extend_from_slice(prefix);
        out.extend_from_slice(core);
        for _ in 0..fill {
            out.push(b' ');
        }
        return;
    }
    if zero {
        out.extend_from_slice(prefix);
        let zeros = width
            .saturating_sub(prefix.len())
            .saturating_sub(core.len());
        for _ in 0..zeros {
            out.push(b'0');
        }
        out.extend_from_slice(core);
    } else {
        for _ in 0..fill {
            out.push(b' ');
        }
        out.extend_from_slice(prefix);
        out.extend_from_slice(core);
    }
}

/// Split a leading `-` off a rendered word into `(sign, rest)`.
pub(super) fn split_sign(word: &str) -> (&[u8], &[u8]) {
    let b = word.as_bytes();
    match b.split_first() {
        Some((c, rest)) if *c == b'-' => (core::slice::from_ref(c), rest),
        _ => (&[], b),
    }
}
