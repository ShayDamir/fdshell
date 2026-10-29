//! The crate's one warning printer (task #364). A warning is not an error, so it
//! never rides a `Report` return channel: it is written straight to stderr.

use crate::io::Stderr;
use alloc::string::String;
use core::fmt::Write;

/// Warn that a here-doc was delimited by the end of the text — bash reads the body
/// to end-of-file, warns, and runs the command at rc 0. The wording is fdshell's
/// own (STYLE §4.7); the delimiter renders with the user-facing lossy-UTF-8
/// convention (`ShortCStr`'s `Display` does the same for error messages), and no
/// line number is claimed because the parse layer decides per statement and cannot
/// reliably know the script line.
pub(crate) fn here_doc_eof(delim: &[u8]) {
    let delim = String::from_utf8_lossy(delim);
    let _ = writeln!(
        Stderr,
        "warning: here-doc delimited by end-of-file (wanted '{delim}')"
    );
}
