//! Keyword-block opacity in the cond-list boundary scan: an `&&` / `||`
//! written inside a keyword block's text (`if`/`for`/`while`/`until`/`case`/
//! `wait`) belongs to the block's *own* cond list — the block executors run a
//! condition through `run_cond_list`, and a body is parsed inside the block —
//! so the outer list must never split a block text in half: `while [ 1 ]`
//! reaches the parser without its `do`/`done`.
//!
//! The scan is a depth counter fed by the shared keyword scorer, the same
//! rule `comment::depth_delta` uses to find a block's span; the `{ …; }`
//! group already has its own opacity counter in `cond/boundary.rs`. A word is
//! scored only where a command word may sit — bash recognizes a block keyword
//! in command position only, and so does this counter (see [`word_delta`]).

use crate::keywords::{first_word_end, keyword_delta};
use crate::segment::group::{group_at_run, opens};

/// The block-depth delta of the word that starts at `i`. Inside a keyword
/// block (`block_depth > 0`) every keyword scores [`keyword_delta`]'s own
/// value, braces included: a literal `{`/`}` argument eats the block's closing
/// keyword there exactly as it does in `comment::depth_delta`
/// (`tests/group.rs::literal_brace_argument_in_a_block_body`), and a nested
/// `if` keeps scoring in the block's clause positions — which this scan cannot
/// see, so it must not gate them.
/// At top level (`block_depth == 0`) only a word in **command position** is a
/// keyword, judged by the segment scanner's own rule [`group_at_run`]: the run
/// is empty, or it ends with `&&` / `||` (a `time [-p]` prefix is transparent
/// there too). A keyword-shaped *argument* (`echo if && echo b`,
/// `echo x > if && echo b`, `echo x | echo if && echo b`) is word data, exactly
/// as bash and the segment scanner require, so its `&&` keeps splitting. A
/// group opener scores 0 too: its depth is counted by `next_boundary`'s group
/// arm, which applies the same clause-position rule, so `echo {` stays word
/// data and splits on `&&`.
pub(super) fn word_delta(line: &[u8], i: usize, block_depth: i32, run_start: usize) -> i32 {
    let tail = line.get(i..).unwrap_or(b"");
    let word_end = first_word_end(tail, i);
    let word = line.get(i..word_end).unwrap_or(b"");
    let at_top_level = block_depth == 0;
    if at_top_level && (opens(word, line, word_end) || group_at_run(line, run_start, i).is_none()) {
        return 0;
    }
    keyword_delta(word, line, word_end).unwrap_or(0)
}

#[cfg(test)]
mod tests;
