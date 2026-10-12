//! Keyword-block opacity in the cond-list boundary scan: an `&&` / `||`
//! written inside a keyword block's text (`if`/`for`/`while`/`until`/`case`/
//! `wait`) belongs to the block's *own* cond list — the block executors run a
//! condition through `run_cond_list`, and a body is parsed inside the block —
//! so the outer list must never split a block text in half: `while [ 1 ]`
//! reaches the parser without its `do`/`done`.
//!
//! The scan is a depth counter fed by the shared keyword scorer, the same
//! rule `comment::depth_delta` uses to find a block's span; the `{ …; }`
//! group already has its own opacity counter in `cond/boundary.rs`.

use crate::keywords::{first_word_end, keyword_delta};
use crate::segment::group::opens;

/// The block-depth delta of the word that starts at `i`. Inside a keyword
/// block (`block_depth > 0`) every keyword scores [`keyword_delta`]'s own
/// value, braces included: a literal `{`/`}` argument eats the block's closing
/// keyword there exactly as it does in `comment::depth_delta`
/// (`tests/group.rs::literal_brace_argument_in_a_block_body`).
/// At top level a group opener scores 0 — its depth is counted by
/// `next_boundary`'s group arm, which applies the segment scanner's
/// clause-position rule, so `echo {` stays word data and splits on `&&`.
pub(super) fn word_delta(line: &[u8], i: usize, block_depth: i32) -> i32 {
    let tail = line.get(i..).unwrap_or(b"");
    let word_end = first_word_end(tail, i);
    let word = line.get(i..word_end).unwrap_or(b"");
    if block_depth == 0 && opens(word, line, word_end) {
        // The group's clause-position rule owns a top-level opener, and a group
        // body never reaches here: `group_depth > 0` already hides the operator.
        return 0;
    }
    keyword_delta(word, line, word_end).unwrap_or(0)
}

#[cfg(test)]
mod tests;
