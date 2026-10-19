mod attached;
mod bare;
mod clobber;
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

/// A redirect token: a bare operator (`>`, `>>`, `<`, `<>`, `>|`, with an
/// optional numeric fd prefix) takes the next token as its path operand; an
/// attached operator is parsed in place. Returns the redirect and how many
/// following tokens the operator consumes (1 for the bare form).
pub fn parse_redirect(
    tokens: &[Token],
    i: usize,
) -> Result<Option<(RedirectDef, usize)>, Report<ParseError>> {
    let Some((t, _start, _end, fq, mask, _spans)) = tokens.get(i) else {
        return Ok(None);
    };
    if *fq {
        return Ok(None);
    }
    if bare::is_bare(t) {
        return bare::parse_bare(tokens, i).map(|def| def.map(|d| (d, 1)));
    }
    Ok(attached::attached(t, mask)?.map(|def| (def, 0)))
}
