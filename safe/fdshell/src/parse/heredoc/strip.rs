//! Line continuations of a here-doc body: the `\<newline>` pairs the body's
//! substitution never sees, because a body has no line structure left by then.

use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

/// Drop the body's line continuations: every `\<newline>` pair is deleted,
/// both bytes. POSIX #4.2 makes an expandable body a **quoted context** —
/// `\<char>` keeps its backslash except `\$`, `\\`, `` \` `` and `\<newline>`,
/// and the newline pair is the only one this pass can see (the other three are
/// the substitution's, and `substitute/arg.rs` is mask-driven).
///
/// Pairs are matched left to right, exactly as the substitution's quoted arm
/// matches them, so `\\` before a newline is an escaped backslash and that
/// newline stays a body byte: body `a\` + newline + `b` is the continuation
/// `ab`, body `a\\` + newline + `b` is two lines (bash agrees, pinned in
/// `tests/heredoc.rs`). A trailing `\` is copied verbatim: the end-of-body
/// backslash belongs to the substitution, which keeps it.
pub(crate) fn strip_continuations(body: &ShortCStr) -> Result<ShortCStr, Report<ParseError>> {
    let bytes = body.as_bytes().change_context(ParseError::Never)?;
    let mut out = ShortCStr::new();
    let mut i = 0usize;
    while let Some(&b) = bytes.get(i) {
        match (b, bytes.get(i + 1)) {
            // A `\<newline>` pair is dropped whole.
            (b'\\', Some(&b'\n')) => {
                i += 2;
                continue;
            }
            // A `\\` pair is copied whole, so its newline can never read as
            // a continuation.
            (b'\\', Some(&b'\\')) => {
                out.push_byte(b'\\')
                    .change_context(ParseError::InvalidChar { ch: 0 })?;
                out.push_byte(b'\\')
                    .change_context(ParseError::InvalidChar { ch: 0 })?;
                i += 2;
                continue;
            }
            _ => out
                .push_byte(b)
                .change_context(ParseError::InvalidChar { ch: 0 })?,
        }
        i += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
