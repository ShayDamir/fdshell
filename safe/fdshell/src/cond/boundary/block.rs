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

use crate::keywords::{Glue, Place, after, first_word_end, keyword_delta, scores};
use crate::segment::group::group_at_run;

mod glue;

/// The block-depth delta of the word that starts at `i`, and what that word
/// does to the scan's command-position flag.
///
/// One clause rule — bash recognizes a reserved word in command position
/// *only* — carried by two halves: at top level the place is read from the
/// word's run by the segment scanner's own rule [`group_at_run`] (its
/// redirect, assignment and pipeline halves included); inside a block it is
/// the scan's [`Place`] flag ([`after`]/[`scores`], task #236), because a
/// run cannot see the clause keywords (`then`/`do`/`else`/`elif`) that
/// restart command position mid-run. Both halves answer the same rule: a
/// keyword-shaped *argument* (`echo if && echo b`, `echo done`) is word
/// data at every depth and its `&&` keeps splitting (task #137), a keyword
/// in a clause position scores, and a *closer* in command position closes
/// the block — including the block a glued cond pair's payload opened
/// (`a||if :; then :; fi`): the pair word's *payload* is scored at its own
/// bytes, and its closer must close, or the `fi` swallows the rest of the
/// line (task #236's S2). Braces are scored by neither: the group arm of
/// `next_boundary` counts them with the same clause rule, so an argument
/// brace (`echo {`, `echo }`) stays word data and `echo { && echo b` keeps
/// splitting.
pub(super) fn word_delta(
    line: &[u8],
    i: usize,
    block_depth: i32,
    run_start: usize,
    place: Place,
) -> (i32, Place) {
    let tail = line.get(i..).unwrap_or(b"");
    let word_end = first_word_end(tail, i);
    let word = line.get(i..word_end).unwrap_or(b"");
    // The byte-level scan meets a glued cond pair twice: its first byte
    // opens the pair word (`||if`), its tail byte starts the tail word
    // (`&if`, `|if`) — the pair's glue position for the transition in
    // [`after`] and for scoring the *payload* at its own bytes (task #236's
    // S2: the glued pair consumes its keyword in the scan's advance, so the
    // keyword behind it must be scored from inside the pair word).
    let strip = glue::strip_at(line, i);
    let glue = match strip {
        0 => Glue::None,
        1 => Glue::Tail,
        _ => Glue::Head,
    };
    // Scoring uses the *word*: [`keyword_delta`]'s pair-tail rule reads the
    // payload behind a glued pair itself, so the scan scores it exactly once
    // (task #236's S2).
    // At top level the place is the run's own answer — the segment scanner's
    // rule [`group_at_run`], with its redirect and pipeline halves; inside a
    // block it is the scan's [`Place`] flag, because a run cannot see the
    // clause keywords (`then`/`do`/`else`/`elif`) that restart command
    // position mid-run. Braces are scored by neither: the group arm of
    // `next_boundary` counts them with the same clause rule, so an argument
    // brace (`echo {`, `echo }`) stays word data (task #236).
    let at = if block_depth == 0 {
        group_at_run(line, run_start, i + strip).map_or(Place::Data, |_| Place::Command)
    } else {
        place
    };
    let next = after(word, line, word_end, at, glue);
    let brace = matches!(word.trim_ascii(), b"{" | b"}");
    let delta = if !brace && scores(word, line, word_end, at) {
        keyword_delta(word, line, word_end).unwrap_or(0)
    } else {
        0
    };
    (delta, next)
}

#[cfg(test)]
mod tests;
