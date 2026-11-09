// mutants: -C --lib -C --test -C group -- -E 'test(/^parse::group_block::/) | binary(group)'

//! The group arm's `;`-before-`}` predecessor: extracted for the line budget.
//!
//! Its base is the shared closer-predecessor set ([`is_closer_predecessor`]: the
//! command-list terminator **plus** the closer of a compound command — the rule
//! task #301 measures, which the group's `}` adopted for task #425). The group's
//! **extra half** is the comment line: bash's brace-group grammar reads the newline
//! that ends a `#` comment as the body's terminator (`{ echo a # c` + newline + `}`
//! runs, rc 0), while its keyword closers do not (`if true; then # c` + newline +
//! `fi` stays bash's syntax error, pinned in `tests/block_bodies.rs`) — so that half
//! of the rule belongs to the group arm, not to `semi::preceded`'s shared set.

use crate::parse::Token;
use crate::parse::semi::is_closer_predecessor;
use crate::scan::{Boundary, ScanState, boundary};

/// The token in front of the group's `}`: the shared closer-predecessor set
/// ([`is_closer_predecessor`]), **plus** a word whose line is closed by a `#` comment.
/// Comment text never reaches the token list — the tokenizer drops a comment and the
/// newline that ends it with it — so this half of the rule reads the raw bytes of the
/// gap the tokenizer left between the word and the closer. Those bytes sit outside every
/// quote (what is quoted, escaped or substituted is inside a token), so the shared walk
/// starts fresh and the `#` it finds is a run's first byte after whitespace: exactly the
/// word-start rule [`boundary`] applies, so no byte compare is needed here.
pub(super) fn terminates_closer(tokens: &[Token], line: &[u8], idx: usize) -> bool {
    let Some(&Token {
        start: prev_end, ..
    }) = tokens.get(idx)
    else {
        return false;
    };
    let Some(&Token {
        start: close_start, ..
    }) = tokens.get(idx + 1)
    else {
        return false;
    };
    is_closer_predecessor(tokens, line, idx) || comment_in_gap(line, prev_end, close_start)
}

/// A word-start `#` comment sits in `line[from..to]`: the gap between the body's last
/// word and the closing brace. A comment running to end of input cannot be in a gap at
/// all — the closer would sit inside it and there would be no `}` token — so the walk
/// needs no extra "the comment is terminated by a newline" test.
fn comment_in_gap(line: &[u8], mut i: usize, to: usize) -> bool {
    let mut state = ScanState::new();
    while i < to {
        if boundary(line, i, &state) == Boundary::Comment {
            return true;
        }
        i = state.advance(line, i);
    }
    false
}

#[cfg(test)]
mod tests;
