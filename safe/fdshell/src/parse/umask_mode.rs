// mutants: -C --lib -C --test -C umask -- -E 'test(/^parse::umask_mode::/) | binary(umask)'

//! `umask`'s mode operand (task #554): an octal mask, or a chmod-style symbolic
//! mode — POSIX XCU 2.95 takes the `mask` operand "in the same way as the `mode`
//! operand … for `chmod`", which is what bash 5.3.9 implements.
//!
//! A word whose first byte is a digit is octal (fdshell's own `0o` prefix
//! extension, task #472, stays); every other word is symbolic. A symbolic mode is
//! only a **plan** here: it is applied to the complement of the *current* mask, so
//! it cannot be evaluated in the parser, which also sees quoted words in statements
//! that never run and must stay side-effect free. `run_dispatch::umask` asks the
//! plan to evaluate when the statement actually runs.

pub(crate) mod symbolic;

use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use symbolic::SymbolicMode;

/// The mode operand: an octal mask, or a symbolic mode to evaluate at run time.
#[derive(Debug, PartialEq)]
pub(crate) enum UmaskMode {
    Octal(u32),
    Symbolic(SymbolicMode),
}

impl UmaskMode {
    /// Parse one operand word; bash's `DIGIT` test on the first byte decides which
    /// of the two grammars the word belongs to.
    pub(crate) fn parse(word: &ShortCStr) -> Result<Self, Report<ParseError>> {
        let bytes = word.as_bytes().change_context(ParseError::Never)?;
        if matches!(bytes, [b'0'..=b'9', ..]) {
            return octal(word);
        }
        Ok(Self::Symbolic(SymbolicMode::parse(word)?))
    }
}

/// The octal operand, exactly as task #472 left it: `from_str_radix` on the digits,
/// with fdshell's `0o` prefix accepted (bash answers `octal number out of range`).
fn octal(word: &ShortCStr) -> Result<UmaskMode, Report<ParseError>> {
    let bytes = word.as_bytes().change_context(ParseError::Never)?;
    let digits = core::str::from_utf8(bytes).change_context(ParseError::InvalidChar { ch: 0 })?;
    let digits = digits.strip_prefix("0o").unwrap_or(digits);
    Ok(UmaskMode::Octal(
        u32::from_str_radix(digits, 8).change_context(ParseError::InvalidOctal)?,
    ))
}

#[cfg(test)]
mod tests;
