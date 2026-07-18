//! Infallible stdout/stderr writers for `core::fmt`.
//!
//! Wraps `sys::OUT` and `sys::ERR` into types implementing
//! `core::fmt::Write`, usable with the `write!`/`writeln!` macros.

use sys::importedfd_io::ImportedFdIo;

/// Writer backed by standard output (fd 1).
pub struct Stdout;

/// Writer backed by standard error (fd 2).
pub struct Stderr;

impl core::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        sys::OUT
            .write_all(s.as_bytes())
            .map_err(|_| core::fmt::Error)
    }
}

impl core::fmt::Write for Stderr {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        sys::ERR
            .write_all(s.as_bytes())
            .map_err(|_| core::fmt::Error)
    }
}
