//! Infallible stderr writer for `core::fmt`.
//!
//! Wraps `sys::ERR` into a type implementing
//! `core::fmt::Write`, usable with the `write!`/`writeln!` macros.

/// Writer backed by standard error (fd 2).
pub struct Stderr;

// `fmt::Write::write_str` must return `fmt::Error`; the underlying
// `SyscallError` cannot be propagated through the trait signature, only
// discarded — see STYLE.md §4.4.
#[allow(clippy::map_err_ignore)]
impl core::fmt::Write for Stderr {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        sys::ERR
            .write_all(s.as_bytes())
            .map_err(|_| core::fmt::Error)
    }
}
