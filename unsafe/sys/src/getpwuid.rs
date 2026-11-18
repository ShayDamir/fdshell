// mutants: -C --test -C getpwuid -- -E 'binary(getpwuid)'

//! `getpwuid(3)` — the password-database lookup behind the prompt's `\u`; the
//! value lives in `sys` because the safe crates never call libc.

use alloc::vec::Vec;
use core::ffi::CStr;

/// `getpwuid(3)` — the login name for `uid`, NUL-trimmed. `None` when there is
/// no entry: glibc returns NULL and leaves errno alone, so "no entry" is the
/// answer, not an error the user fixes (STYLE §4.12) — the prompt layer decides
/// the display.
pub fn getpwuid(uid: u32) -> Option<Vec<u8>> {
    // SAFETY: `getpwuid` accepts any `uid_t` value; it returns NULL with no
    // entry, else a pointer to a static `passwd` object it owns.
    let pw = unsafe { libc::getpwuid(uid as libc::uid_t) };
    if pw.is_null() {
        return None;
    }
    // SAFETY: `pw` is non-null and a real entry's `pw_name` points to a
    // NUL-terminated C string owned by libc; copied out immediately.
    Some(unsafe { CStr::from_ptr((*pw).pw_name) }.to_bytes().to_vec())
}
