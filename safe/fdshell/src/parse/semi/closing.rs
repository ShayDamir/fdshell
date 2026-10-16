//! The closing-keyword search of `parse/semi.rs`, gated by the shared
//! command-position rule (task #236).

use super::Token;
use crate::keywords::{Place, after, keyword_delta, scores};

/// The index of the block's closing keyword: the token at which the block
/// depth (starting at 1 for the opening keyword) first returns to 0. Tokens
/// after it are heredoc body data (a condition-position heredoc's bodies sit
/// in the block text after the closing keyword), not block structure.
///
/// The scan carries bash's command-position rule over the token list
/// ([`after`]/[`scores`], task #236): a block keyword written as an argument
/// (`echo done` in a `do` body) never closes the block — the `;` before a
/// real closer is what restarts the flag. Braces stay ungated, as every
/// scorer of this rule leaves a literal `{`/`}` argument to the accepted
/// divergence `tests/group.rs::literal_brace_argument_in_a_block_body` pins.
pub(crate) fn closing_keyword_index(tokens: &[Token], line: &[u8]) -> Option<usize> {
    let mut depth: i32 = 1;
    let mut place = Place::Command;
    for (i, (t, _, end, _, _)) in tokens.iter().enumerate().skip(1) {
        let Some(word) = t.as_bytes().ok() else {
            continue;
        };
        let scored = scores(word, line, *end, place);
        place = after(word, line, *end, place);
        let Some(delta) = scored.then(|| keyword_delta(word, line, *end)).flatten() else {
            continue;
        };
        depth += delta;
        if depth == 0 {
            return Some(i);
        }
    }
    None
}
