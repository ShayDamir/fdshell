//! Heredoc parsing: extracting the bodies of `<<` operators from a line and
//! turning the operator tokens into stdin redirects.

mod eof;
mod operator;
mod strip;

use super::Token;
use crate::error::parse::ParseError;
use crate::redirect::RedirectDef;
use crate::scan::heredoc;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(crate) use operator::{delimiter_token_indices, is_operator, operator_count, operators};

/// One parsed heredoc, in command order.
#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub(crate) struct HeredocBody {
    /// The raw body bytes (the lines between the command line and the
    /// delimiter line, including the last body line's newline).
    pub(crate) body: ShortCStr,
    /// Whether `$` / backtick expansions run in the body (unquoted delimiter).
    pub(crate) expand: bool,
}

/// Extract every heredoc body of the line, in operator order.
pub(crate) fn layout(
    line: &[u8],
    tokens: &[Token],
) -> Result<Vec<HeredocBody>, Report<ParseError>> {
    let ops = operator::operators(line, tokens)?;
    if ops.is_empty() {
        return Ok(Vec::new());
    }
    // Both shapes the old version rejected are bash's end-of-file here-doc now
    // (task #364): nothing left after the command line means an empty body, the
    // lines that are left mean a body, and `eof` prints the warning.
    let from = heredoc::first_unquoted_newline(line).unwrap_or(line.len());
    let (spans, _) = heredoc::body_spans(line, from, &ops);
    eof::warn_eof(&spans, &ops);
    ops.iter()
        .zip(spans)
        .map(|(op, span)| {
            let raw = line
                .get(span.region.0..span.region.1)
                .ok_or(ParseError::Never)?;
            // Bash reads body lines whole, so an end-of-file body gets the
            // newline its last line lacks (`close_last_line`); a found span
            // keeps its bytes byte-exact.
            let bytes = if span.found {
                op.body_bytes(raw)
            } else {
                op.body_bytes(&eof::close_last_line(raw.to_vec()))
            };
            let body =
                ShortCStr::from_vec(bytes).change_context(ParseError::InvalidChar { ch: 0 })?;
            // An expandable body is a quoted context whose `\<newline>` pairs
            // are line continuations: they are gone before the substitution
            // (POSIX #4.2, `strip::strip_continuations`). A quoted body is
            // verbatim bytes and keeps its backslashes.
            let body = if op.quoted {
                body
            } else {
                strip::strip_continuations(&body)?
            };
            Ok(HeredocBody {
                body,
                expand: !op.quoted,
            })
        })
        .collect()
}

/// Turn the operator at `i` into a redirect from its pre-extracted body: the
/// body goes to the operator's own descriptor (`0` for `<<T`, the io-number
/// of `3<<T`).
pub(crate) fn parse_operator(
    line: &[u8],
    tokens: &[Token],
    i: usize,
    spec: &HeredocBody,
) -> Result<(RedirectDef, usize), Report<ParseError>> {
    let (op, extra) = operator::operator_at(line, tokens, i)?;
    Ok((
        RedirectDef::here_doc(op.fd, spec.body.clone(), spec.expand),
        extra,
    ))
}

#[cfg(test)]
mod tests;
