//! The gate scan of a short-circuited cond-list operator (task #236): the
//! byte just past the `;` that ends the gated AND-list.

use crate::keywords::Place;
use crate::scan::ScanState;
use crate::segment::group::opens_at;

use super::{block, body, opens_group};

/// The byte just past the separator that ends the gated AND-list starting at
/// `from` (or `line.len()`): bash gates the *AND-list* a `;` ends, not the
/// rest of the statement (task #236), so a short-circuited `||` chain skips
/// to here and the list resumes ungated behind it — `echo a || if :; then
/// :; fi; break` skips the `if` and runs `break`. The scan is the region
/// rule of a *statement's* text: keyword blocks are opaque (their `;`s are
/// body bytes, counted by the shared [`block::word_delta`] scorer so the
/// block's closer ends the depth), a `{ …; }` group is opaque, and heredoc
/// bodies are skipped whole; no operator splits — only the depth-0 separator
/// ends the gate.
pub(crate) fn gate_end(line: &[u8], from: usize, state: &mut ScanState) -> usize {
    let mut i = from;
    let mut run_start = from;
    let mut group_depth: i32 = 0;
    let mut block_depth: i32 = 0;
    let mut place = Place::Command;
    // The gated list starts right behind the operator pair, at a fresh command
    // word: the boundary scan's word state must not swallow it (task #236).
    state.word_active = false;
    // The gate's block depth must *close*: the block's closers are scored by
    // the same run-rule scorer the boundary scan uses — a block opened
    // inside the gated text ends at its closer, and the `;` behind it ends
    // the gate.
    while let Some(&b) = line.get(i) {
        if !state.in_quote && !state.in_single && state.paren_depth == 0 {
            if state.at_word_start() {
                let (delta, next) = block::word_delta(line, i, block_depth, run_start, place);
                block_depth = block_depth.saturating_add(delta).max(0);
                place = next;
            }
            if opens_at(line, i) && opens_group(line, run_start, i) {
                group_depth += 1;
            } else if b == b'}' {
                group_depth = group_depth.saturating_sub(1).max(0);
            }
            if b == b'\n'
                && let Some(body::Step::Skip(resume)) = body::step(line, run_start, i, block_depth)
            {
                // Heredoc bodies are opaque data: the gate scan never splits
                // inside one, it resumes at the last delimiter's newline.
                i = resume;
                run_start = i;
                state.word_active = false;
                continue;
            }
            if b == b'\n' || b == b';' {
                if group_depth == 0 && block_depth == 0 {
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
