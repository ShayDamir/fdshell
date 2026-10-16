//! `&&` / `||` boundary scanning for cond lists, skipping heredoc bodies so
//! a multi-line heredoc part stays one part and `&&` / `||` in its body do
//! not split it. Inside a keyword block the block's command text is opaque,
//! while the heredoc **body** regions it carries inline stay scanned
//! (`boundary/body.rs`).

use crate::scan::ScanState;
use crate::segment::group::{group_at_run, opens_at};

mod block;
mod body;

/// Scan from `i` to the next unquoted `&&` / `||` (or end of line). Heredoc
/// command lines are skipped whole along with their bodies. A `{ …; }` group
/// is opaque at this level too: the braces are counted, and an operator inside
/// the group's body is part of the body (the group's own text), so the split
/// happens after the closing brace. A keyword block is opaque the same way:
/// `block_depth` counts its keywords, so an operator written in a loop
/// condition, an `if` condition, a body or a `case` arm belongs to the block's
/// own cond list. Both counters only recognize a *command-position* word — a
/// keyword-shaped argument (`echo if && echo b`) is word data, as bash and the
/// segment scanner require. A block that carries heredoc bodies in its own
/// text (a block condition's bodies travel inside the span) keeps the block
/// rule, and its **body** regions are walked region by region: skipped whole
/// in a statement's text, scanned for an operator inside a block's text
/// ([`body::step`]). Returns the operator's first byte (or
/// `line.len()`) and whether it is `||`.
pub(crate) fn next_boundary(line: &[u8], mut i: usize, state: &mut ScanState) -> (usize, bool) {
    let mut run_start = i;
    let mut group_depth: i32 = 0;
    let mut block_depth: i32 = 0;
    while let Some(&b) = line.get(i) {
        if !state.in_quote && !state.in_single && state.paren_depth == 0 {
            if group_depth == 0
                && block_depth == 0
                && ((b == b'&' && line.get(i + 1) == Some(&b'&'))
                    || (b == b'|' && line.get(i + 1) == Some(&b'|')))
            {
                return (i, b == b'|');
            }
            // Only a word start can open a block; the closers that end one are
            // scored by the same shared helper. A stray closer clamps at 0, so
            // a stray `done` never hides the operators of a plain command line,
            // and the helper's command-position rule keeps a keyword-shaped
            // *argument* (`echo if && echo b`) word data too.
            if state.at_word_start() {
                let delta = block::word_delta(line, i, block_depth, run_start);
                block_depth = block_depth.saturating_add(delta).max(0);
            }
            // Only a brace that opens a group counts: at a cond-list part start
            // (the run is empty, or the run ends with `&&` / `||`). A brace in an
            // argument position (`echo x{`, `echo {`) is word data, exactly as the
            // segment scanner's `group_at_run` rule requires.
            if opens_at(line, i) && group_at_run(line, run_start, i).is_some() {
                group_depth += 1;
            } else if b == b'}' {
                // A stray closer must clamp at 0, not go negative: a negative
                // depth would make the `group_depth == 0` test false for every
                // later `&&`/`||` and swallow the operator into the arguments.
                group_depth = group_depth.saturating_sub(1).max(0);
            }
            if b == b'\n'
                && let Some(step) = body::step(line, run_start, i, block_depth)
            {
                match step {
                    // The block's own body region carries the operator: it is
                    // the cond-list boundary.
                    body::Step::Split(at, is_or) => return (at, is_or),
                    // Every region is opaque: resume at the newline terminating
                    // the last delimiter line. Its run reset happens in the plain
                    // arm below, where the scan meets that newline again.
                    body::Step::Skip(resume) => {
                        i = resume;
                        run_start = i;
                        // The run reset starts a new word: the line after a
                        // heredoc body begins with a command word.
                        state.word_active = false;
                        continue;
                    }
                }
            }
            if b == b'\n' || b == b';' {
                i += 1;
                run_start = i;
                // A run reset is a word reset: a closer written at the start
                // of a physical line must be scored, or the block's trailing
                // operator is swallowed into its text.
                state.word_active = false;
                continue;
            }
        }
        i = state.advance(line, i);
    }
    (line.len(), false)
}
