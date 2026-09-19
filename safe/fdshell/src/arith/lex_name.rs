//! Name and `$` lexing: bare names, `$name`, `$$` (PID), `$!` (last bg PID).

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

use super::lex::Tok;
use crate::error::resolve::ResolveError;

pub(super) fn lex_name(
    body: &[u8],
    i: usize,
    toks: &mut Vec<Tok>,
) -> Result<usize, Report<ResolveError>> {
    let (name, j) = collect_name(body, i)?;
    toks.push(Tok::Name(name));
    Ok(j)
}

/// `$name` (a variable reference), `$$` (shell PID), `$!` (last background PID).
pub(super) fn lex_dollar(
    body: &[u8],
    i: usize,
    toks: &mut Vec<Tok>,
) -> Result<usize, Report<ResolveError>> {
    match body.get(i + 1) {
        Some(&b'$') => {
            toks.push(Tok::Pid);
            Ok(i + 2)
        }
        Some(&b'!') => {
            toks.push(Tok::LastBg);
            Ok(i + 2)
        }
        Some(&c) if c.is_ascii_alphabetic() || c == b'_' => {
            let (name, j) = collect_name(body, i + 1)?;
            toks.push(Tok::Name(name));
            Ok(j)
        }
        // `$` before a digit, `(`, `)`, or any other byte is a syntax error
        // in v1 (no positional parameters, command substitution, or `${…}`
        // inside arithmetic).
        _ => bail!(ResolveError::ArithSyntax),
    }
}

/// Collect an alphanumeric/`_` run starting at `i`.
fn collect_name(body: &[u8], i: usize) -> Result<(ShortCStr, usize), Report<ResolveError>> {
    let mut name = ShortCStr::new();
    let mut j = i;
    while let Some(&c) = body.get(j) {
        if !(c.is_ascii_alphanumeric() || c == b'_') {
            break;
        }
        // Name bytes are alphanumeric, so a NUL push is impossible.
        name.push_byte(c).change_context(ResolveError::Never)?;
        j += 1;
    }
    Ok((name, j))
}
