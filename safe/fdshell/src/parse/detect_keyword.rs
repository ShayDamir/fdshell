// mutants: -C --lib -C --test -C umask -- -E 'test(/^parse::detect_keyword::/) | binary(umask)'

use super::Token;
use crate::error::parse::ParseError;
use crate::parse::line::ParsedLine;
use error_stack::{Report, ResultExt, bail, ensure};
use sys::ShortCStr;

pub(crate) fn detect_control(tokens: &[Token]) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let first = match tokens.first() {
        Some(Token { word: t, .. }) => t,
        None => return Ok(None),
    };

    if first.eq_bytes(b"break") {
        ensure!(
            tokens.get(2).is_none(),
            ParseError::BreakContinueTakesAtMostOneArgument
        );
        return Ok(Some(ParsedLine::Break(loop_count(tokens)?)));
    }

    if first.eq_bytes(b"continue") {
        ensure!(
            tokens.get(2).is_none(),
            ParseError::BreakContinueTakesAtMostOneArgument
        );
        return Ok(Some(ParsedLine::Continue(loop_count(tokens)?)));
    }

    if first.eq_bytes(b"return") {
        return detect_return(tokens);
    }

    Ok(None)
}

/// The optional `break`/`continue` count: 1 when absent. `u32::from_str` accepts
/// a leading `+` and leading zeros (bash's `atoi` agrees) and rejects `-1`,
/// `1.5`, `0x2` and overflow; `0` is rejected here (POSIX `break 0` means "exit
/// the shell execution environment", which this shell cannot express).
fn loop_count(tokens: &[Token]) -> Result<u32, Report<ParseError>> {
    match tokens.get(1) {
        None => Ok(1),
        Some(Token { word: t, .. }) => {
            let n = t
                .parse::<u32>()
                .change_context(ParseError::LoopCountInvalid)?;
            ensure!(n > 0, ParseError::LoopCountInvalid);
            Ok(n)
        }
    }
}

/// `return` with an optional integer status.
pub(crate) fn detect_return(tokens: &[Token]) -> Result<Option<ParsedLine>, Report<ParseError>> {
    if tokens.get(2).is_some() {
        bail!(ParseError::ReturnTakesAtMostOneArgument);
    }
    let code = match tokens.get(1) {
        None => None,
        Some(Token { word: t, .. }) => Some(parse_status(t)?),
    };
    Ok(Some(ParsedLine::Return(code)))
}

fn parse_status(t: &ShortCStr) -> Result<i32, Report<ParseError>> {
    t.parse::<i32>().change_context(ParseError::InvalidInt)
}

/// `umask`'s operand and option scan lives in its own module too: the operand has
/// two grammars, and the symbolic one is evaluated only when the statement runs.
pub(crate) mod umask;

/// `unset`'s operands live in their own module: the list is infallible, so the
/// detector returns the statement directly (`pub(crate)` for `parse::detect`).
pub(crate) mod unset;

#[cfg(test)]
mod tests;
