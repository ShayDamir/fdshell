//! Heredoc parsing: extracting the bodies of `<<` operators from a line and
//! turning the operator tokens into stdin redirects.

mod operator;

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
    let from = heredoc::first_unquoted_newline(line)
        .ok_or_else(|| unterminated(ops.first().map(|o| o.delim).unwrap_or(b"")))?;
    let (spans, _) = heredoc::body_spans(line, from, &ops)
        .map_err(|n| unterminated(ops.get(n).map(|o| o.delim).unwrap_or(b"")))?;
    ops.iter()
        .zip(spans)
        .map(|(op, (body_start, body_end))| {
            let bytes = op.body_bytes(line.get(body_start..body_end).ok_or(ParseError::Never)?);
            let body =
                ShortCStr::from_vec(bytes).change_context(ParseError::InvalidChar { ch: 0 })?;
            Ok(HeredocBody {
                body,
                expand: !op.quoted,
            })
        })
        .collect()
}

/// Turn the operator at `i` into a redirect from its pre-extracted body.
pub(crate) fn parse_operator(
    line: &[u8],
    tokens: &[Token],
    i: usize,
    spec: &HeredocBody,
) -> Result<(RedirectDef, usize), Report<ParseError>> {
    let (_op, extra) = operator::operator_at(line, tokens, i)?;
    Ok((RedirectDef::here_doc(spec.body.clone(), spec.expand), extra))
}

fn unterminated(delim: &[u8]) -> Report<ParseError> {
    // A NUL in the delimiter is impossible: the delimiter is a token word
    // and tokens never carry NUL bytes.
    let Ok(delim) = ShortCStr::from_vec(delim.to_vec()) else {
        return Report::new(ParseError::Never);
    };
    Report::new(ParseError::UnterminatedHeredoc { delim })
}

#[cfg(test)]
mod tests;
