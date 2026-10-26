//! bash's `worddef` rule applied to the brace-expansion rewrite: a redirection
//! operand expanded to more than one word — or to none — is bash's
//! `ambiguous redirect`, and the pair *(word as written, expanded words)*
//! exists **only** here, inside the pre-parse rebuild. After the rewrite the
//! text is re-tokenized and the parse layer cannot tell `cat <&3 <&4` from a
//! script that wrote exactly that, so the rule is decided before the parse
//! (and therefore before anything is opened, as bash does).

use super::word::expand_word;
use crate::error::cmd::CmdError;
use crate::parse::operand_offset;
use crate::parse::Token;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

/// Refuse token `i`'s expansion when the **operand region** of its redirect
/// expands to a number of non-empty words other than one. Bash counts
/// non-empty words (POSIX XCU 2.7 `worddef`): `{,zz}` leaves `zz` and stays
/// legal, `{,}` leaves nothing and is ambiguous.
///
/// The region starts at `operand_offset` — the parser's own answer to "where
/// does the operand begin" — so braces in an io-number prefix are outside the
/// count for free and the message names the operand as bash writes it. Here
/// `expand_word` runs on the region a **second** time (the first pass produced
/// the words being appended, and only their count matters): the guard is reached
/// only for a redirect operand whose word is already multi-word, so the extra
/// pass costs nothing measurable.
pub(super) fn guard(tokens: &[Token], i: usize, span: &[u8]) -> Result<(), Report<CmdError>> {
    let Some(off) = operand_offset(tokens, i) else {
        return Ok(());
    };
    let Some(region) = span.get(off..) else {
        return Ok(());
    };
    let Some(words) = expand_word(region).change_context(CmdError::Parse)? else {
        return Ok(());
    };
    let non_empty = words.iter().filter(|w| !w.is_empty()).count();
    if non_empty == 1 {
        return Ok(());
    }
    let name = ShortCStr::from_vec(region.to_vec()).change_context(CmdError::Never)?;
    bail!(CmdError::AmbiguousRedirect { name });
}

#[cfg(test)]
mod tests;
