//! The **word** written after a keyword block's closing keyword: bash demands a
//! separator there, so the word is its syntax error and this module reports it at
//! the house rc 1 (task #405, the keyword-block twin of `group_block::tail`'s
//! `GroupTrailingWord`, measured against bash 5.3.9). The three operators the walk
//! takes itself — path, here-doc ([`super::heredoc`]) and here-string — never reach this
//! judge. The shapes it **stops** at stay their owners': the separator and
//! word-break shapes of the #316/#396 family (which is also what stops a `<<`
//! glued *inside* the closer's word — the pinned house rule "a `<<` glued inside a
//! word stays word data": the byte scan counts no operator there, so the here-doc
//! arm declines it and the walk stops without a report), the comment word of task
//! #296, and the heredoc structure an append leaves behind the closer.

use crate::bytes::word_break;
use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::scan::heredoc::terminator_spans;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

/// Judge the token the tail walk stopped at: `Ok(())` when it is a shape the walk
/// stops at without judging (see [`stops`]), the report when it is a word.
pub(crate) fn judge(kw: &[u8], tok: &Token, line: &[u8]) -> Result<(), Report<ParseError>> {
    if stops(tok, line) {
        return Ok(());
    }
    // A NUL in a closer keyword is an invariant breach (the callers pass the
    // keyword literals), so it answers `Never` (STYLE §4.10) — reached through the
    // cross-crate `.change_context()` (STYLE §4.5), keeping the chain.
    let closer = ShortCStr::from_vec(kw.to_vec()).change_context(ParseError::Never)?;
    bail!(ParseError::BlockTrailingWord { closer });
}

/// Whether the walk stops at `tok` without judging it: the tokenizer's own
/// word-break set [`word_break`] at the token's raw start — the separators, the
/// cond operators, the pipeline `|` and a word **starting** at a break byte
/// (`done (x)`, the #316/#396 family) — a word-start `#` (task #296's comment
/// rule, whose text reaches this walk inside a block's span), and a word of an
/// appended heredoc's **terminator** line (`done <<T` + body + delimiter: the
/// region's own words are blanked by `tokenize_statement`, the delimiter's are
/// not, so it is the structure the group's tail cuts with `command_tail`; here
/// the delimiter's own span says it, since a keyword block has no body-token range
/// to key the group's append/extension pair on). A quoted or escaped shape is word
/// data and reaches the report — the group tail's pinned reading (task #336).
fn stops(tok: &Token, line: &[u8]) -> bool {
    let (_, start, _, _, _, _) = tok;
    terminator_spans(line)
        .iter()
        .any(|(from, to)| (*from..*to).contains(start))
        || line
            .get(*start)
            .is_some_and(|&b| word_break(b) || b == b'#')
}
