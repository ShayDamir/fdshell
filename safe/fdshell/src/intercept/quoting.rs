// mutants: -C --lib -C --test -C alias -C --test -C trap -- -E 'binary(alias) | binary(trap)'

//! The single-quote rule the list forms print with, so what they print can be
//! re-input: `alias` and `trap` share it, and one owner keeps them agreeing
//! (bash 5.3.9, measured: `trap -- 'echo a'\''b' SIGUSR1` re-inputs as itself).

use alloc::vec::Vec;

/// Append `value` to `out` wrapped in single quotes, every embedded `'` rendered
/// as `'\''` — the shell's own re-input quoting. The bytes go in as they are:
/// `Display` on `ShortCStr` is lossy (LESSONS, "`Display` on `ShortCStr` is
/// correct for user-facing error messages; forbidden for data paths"), and a
/// stored action is data.
pub(crate) fn push_quoted(value: &[u8], out: &mut Vec<u8>) {
    out.push(b'\'');
    for &byte in value {
        if byte == b'\'' {
            out.extend_from_slice(b"'\\''");
        } else {
            out.push(byte);
        }
    }
    out.push(b'\'');
}
