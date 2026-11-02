//! The group arm's `;`-before-`}` predecessor: extracted for the line budget.
//!
//! Bash's **brace-group** grammar reads the newline that ends a `#` comment as the
//! body's terminator (`{ echo a # c` + newline + `}` runs, rc 0), while its keyword
//! closers do not (`if true; then # c` + newline + `fi` stays bash's syntax error,
//! pinned in `tests/block_bodies.rs`) — so the half of the rule that reads a comment
//! line belongs to the group arm, not to `semi::preceded`'s shared predecessor set.

use crate::parse::Token;
use crate::parse::semi::is_list_terminator;
use crate::scan::{Boundary, ScanState, boundary};

/// The token in front of the group's `}`: the shared terminator set
/// ([`is_list_terminator`]), **plus** a word whose line is closed by a `#` comment.
/// Comment text never reaches the token list — the tokenizer drops a comment and the
/// newline that ends it with it — so this half of the rule reads the raw bytes of the
/// gap the tokenizer left between the word and the closer. Those bytes sit outside every
/// quote (what is quoted, escaped or substituted is inside a token), so the shared walk
/// starts fresh and the `#` it finds is a run's first byte after whitespace: exactly the
/// word-start rule [`boundary`] applies, so no byte compare is needed here.
pub(super) fn terminates_closer(tokens: &[Token], line: &[u8], idx: usize) -> bool {
    let Some(tok @ (_, prev_end, _, _, _, _)) = tokens.get(idx) else {
        return false;
    };
    let Some(&(_, close_start, _, _, _, _)) = tokens.get(idx + 1) else {
        return false;
    };
    is_list_terminator(tok) || comment_in_gap(line, *prev_end, close_start)
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
