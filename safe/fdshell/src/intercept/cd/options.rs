//! The option-word half of `cd`: the accepted words are parsed on the expanded
//! fields, from the head of the list, and everything after them is the operand
//! list.

use error_stack::{Report, bail};

use crate::error::cmd::CmdError;
use sys::ShortCStr;

/// Walk the leading option words of the **expanded** field list and return the
/// operand list. `-L`, `-P` and `-e` are accepted as no-ops: the move is a pinned
/// `O_DIRECTORY + O_NOFOLLOW` fd (README "The accepted divergences"), so the
/// logical/physical distinction bash measures on a symlinked directory has
/// nothing to decide here, and the conflict bash accepts is accepted too.
pub(super) fn operands(fields: &[ShortCStr]) -> Result<&[ShortCStr], Report<CmdError>> {
    let mut rest = fields;
    while let Some(word) = rest.first() {
        if is_noop_option(word) {
            rest = rest.get(1..).unwrap_or(&[]);
            continue;
        }
        if word.eq_bytes(b"--") {
            return Ok(rest.get(1..).unwrap_or(&[]));
        }
        if word.eq_bytes(b"-") || !word.starts_with(b"-") {
            return Ok(rest);
        }
        bail!(CmdError::CdInvalidOption {
            option: word.clone()
        });
    }
    Ok(rest)
}

/// The accepted words, matched whole — no getopt cluster walker (decision pinned
/// in README: bash's help line `[-@]` is the getopt `@` marker, not an option).
fn is_noop_option(word: &ShortCStr) -> bool {
    word.eq_bytes(b"-L") || word.eq_bytes(b"-P") || word.eq_bytes(b"-e")
}
