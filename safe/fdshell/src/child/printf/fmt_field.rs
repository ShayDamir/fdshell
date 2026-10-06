//! Shared field padding for the `printf` formatters.

use alloc::vec::Vec;

/// Pad `core` to `width` bytes and append it to `out`.
///
/// `left` left-justifies; `zero` zero-fills (only honoured when
/// right-justified); otherwise spaces fill. A `width` at most the core length
/// prints the core unchanged.
pub(super) fn pad(left: bool, zero: bool, width: usize, core: &[u8], out: &mut Vec<u8>) {
    let fill = width.saturating_sub(core.len());
    let byte = if zero && !left { b'0' } else { b' ' };
    if !left {
        for _ in 0..fill {
            out.push(byte);
        }
    }
    out.extend_from_slice(core);
    if left {
        for _ in 0..fill {
            out.push(byte);
        }
    }
}
