//! The gate scan of a short-circuited cond-list operator (task #236): the
//! byte just past the `;` that ends the gated AND-list.

use crate::keywords::{Pairing, Place};
use crate::scan::{ScanState, comment_end, last_op};
use crate::segment::group::opens_at;

use super::{
    block::{self, Modes},
    body, opens_group,
};

/// The byte just past the separator that ends the gated AND-list starting at
/// `from` (or `line.len()`): bash gates the *AND-list* a `;` ends, not the
/// rest of the statement (task #236), so a short-circuited `||` chain skips
/// to here and the list resumes ungated behind it — `echo a || if :; then
/// :; fi; break` skips the `if` and runs `break`. The scan is the region
/// rule of a *statement's* text: keyword blocks are opaque (their `;`s are
/// body bytes, paired by the shared [`block::word_delta`] scorer so the
/// block's own closer ends the pairing), a `{ …; }` group is opaque, and heredoc
/// bodies are skipped whole; no operator splits — only the depth-0 separator
/// ends the gate. A word-start `#` comment runs to the end of its physical
/// line, so a separator written inside it is comment text and the gate runs to
/// that line's newline (task #296).
pub(crate) fn gate_end(line: &[u8], from: usize, state: &mut ScanState) -> usize {
    let mut i = from;
    let mut run_start = from;
    let mut group_depth: i32 = 0;
    let mut pairing = Pairing::new();
    let mut place = Place::Command;
    // The region rule's clause/body mode, fed by this scan's own word walk: the
    // gate never splits, so its answer is `Skip` either way, and the mode keeps
    // the shared `body::step` call honest (task #304).
    let mut modes = Modes::new();
    // The gated list starts right behind the operator pair, at a fresh command
    // word: the boundary scan's word state must not swallow it (task #236).
    state.word_active = false;
    // The gate's block pairing must *close*: the block's closers are scored by
    // the same run-rule scorer the boundary scan uses — a block opened
    // inside the gated text ends at the closer of **its own kind** (task #309),
    // and the `;` behind it ends the gate.
    while let Some(&b) = line.get(i) {
        if !state.in_quote && !state.in_single && state.paren_depth == 0 {
            if state.at_word_start() {
                if b == b'#' {
                    // The gate shares the boundary scan's comment rule: a `#`
                    // at a word start comments out the rest of its physical
                    // line, so no `;` written inside it ends the gate (bash
                    // never resumes ungated inside comment text). Landing **on**
                    // the newline that ends the comment lets the `\n` arm below
                    // end the gate at depth 0 (task #296).
                    i = comment_end(line, i);
                    continue;
                }
                let (delta, next) = block::word_delta(line, i, &mut pairing, run_start, place);
                modes.scan(line, i, place, delta);
                place = next;
            }
            if opens_at(line, i) && opens_group(line, run_start, i) {
                group_depth += 1;
            } else if b == b'}' {
                group_depth = group_depth.saturating_sub(1).max(0);
            }
            if b == b'\n'
                && let Some(body::Step::Skip(resume)) =
                    body::step(line, run_start, i, pairing.is_open(), &modes)
            {
                // Heredoc bodies are opaque data: the gate scan never splits
                // inside one, it resumes at the last delimiter's newline.
                i = resume;
                run_start = i;
                state.word_active = false;
                continue;
            }
            if b == b'\n' || b == b';' {
                // POSIX #2.9's AND-IF-TERMINATOR (task #492): a newline behind a
                // top-level `&&`/`||` is part of the operator token, so it does
                // not end the gated list either — the gate spans it exactly as it
                // spans the same-line spelling. A `;` still ends it.
                let terminator = b == b'\n' && last_op::ends_on_operator(line, i);
                if group_depth == 0 && !pairing.is_open() && !terminator {
                    return i + 1;
                }
                // A `;` inside a block still resets the gate's run: each
                // clause begins a fresh word run, so the closer at the end of
                // the block's last clause is scored on a run that ends with
                // `;` (task #236: `echo a || if :; then :; fi; break` — the
                // `fi` closes the block and *its* `;` ends the gated list).
                i += 1;
                run_start = i;
                place = Place::Command;
                state.word_active = false;
                continue;
            }
        }
        i = state.advance(line, i);
    }
    line.len()
}
