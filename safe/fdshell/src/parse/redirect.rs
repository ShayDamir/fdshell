mod attached;
mod bare;
mod clobber;
pub(crate) mod path_word;
pub(crate) use clobber::{clobber_word, pipe_token_span, pipeline_pipe};

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::redirect::RedirectDef;
use error_stack::Report;
use sys::ShortCStr;

/// The ONE descriptor-number rule: a `N>file`, `N<file` and a `N<<word`
/// prefix all parse here, so a heredoc and a path redirect can never read the
/// same digits as two different numbers. `None` for a run that does not fit
/// `i32`; `dir` only selects the default of an unprefixed operator.
pub(crate) fn parse_fd(prefix: &ShortCStr, dir: u8) -> Option<i32> {
    if prefix.is_empty() {
        Some(match dir {
            b'<' => 0,
            _ => 1,
        })
    } else {
        prefix.parse().ok()
    }
}

/// The operator-quoting gate: POSIX XCU 2.9.1 builds a redirection out of
/// **unquoted** bytes, so a quote byte ends the operator and begins the word.
/// `mask` is the token's per-byte quote mask (one bit per *text* byte, no bit
/// for a stripped quote byte — LESSONS `pos`/`cur`); a slice past its end
/// pads to *unquoted*, the house idiom (`scan::heredoc::op::operator_word`,
/// `substitute/tilde.rs::prefix_quoted`). The operand-side twin of this rule
/// is `operator_word`; no site re-derives either.
pub(crate) fn quoted_bytes(mask: &[bool], from: usize, to: usize) -> bool {
    mask.get(from..to).unwrap_or(&[]).iter().any(|&bit| bit)
}

/// The descriptor a redirect *token* selects: `parse_fd`'s digit rule plus
/// POSIX's quote rule — an io-number is read only from **unquoted** digits,
/// so `'2'>f` has no io-number and `>` takes its default fd 1. A non-digit
/// prefix (letters, `%`, an escaped `\2`) stays "no redirect": fdshell never
/// splits a word at an operator byte (POSIX_COMPATIBILITY "Operators are
/// recognized at word starts").
pub(crate) fn token_fd(prefix: &ShortCStr, mask: &[bool], dir: u8) -> Option<i32> {
    let digits = match prefix.as_bytes() {
        Ok(d) => !d.is_empty() && d.iter().all(u8::is_ascii_digit),
        Err(_) => false,
    };
    let prefix = if digits && quoted_bytes(mask, 0, prefix.len()) {
        ShortCStr::new()
    } else {
        prefix.clone()
    };
    parse_fd(&prefix, dir)
}

/// A redirect token: a bare operator (`>`, `>>`, `<`, `<>`, `>|`, with an
/// optional numeric fd prefix) takes the next token as its path operand; an
/// attached operator is parsed in place. Returns the redirect and how many
/// following tokens the operator consumes (1 for the bare form). A quote
/// byte in the digits or the operator makes the token an ordinary argument:
/// every arm below runs through the `quoted_bytes` gate, which subsumes the
/// `fq` flag (a fully quoted word has every operator byte quoted).
pub fn parse_redirect(
    tokens: &[Token],
    i: usize,
) -> Result<Option<(RedirectDef, usize)>, Report<ParseError>> {
    let Some(token) = tokens.get(i) else {
        return Ok(None);
    };
    let (t, _start, _end, _fq, _mask, _spans) = token;
    if bare::is_bare(t) {
        return bare::parse_bare(tokens, i).map(|def| def.map(|d| (d, 1)));
    }
    Ok(attached::attached(token)?.map(|def| (def, 0)))
}

#[cfg(test)]
mod tests;
