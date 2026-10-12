//! `&&` / `||` boundary scanning for cond lists, skipping heredoc bodies so
//! a multi-line heredoc part stays one part and `&&` / `||` in its body do
//! not split it.

use crate::scan::{ScanState, heredoc};
use crate::segment::group::{group_at_run, opens_at};

mod block;

/// Scan from `i` to the next unquoted `&&` / `||` (or end of line). Heredoc
/// command lines are skipped whole along with their bodies. A `{ …; }` group
/// is opaque at this level too: the braces are counted, and an operator inside
/// the group's body is part of the body (the group's own text), so the split
/// happens after the closing brace. A keyword block is opaque the same way:
/// `block_depth` counts its keywords, so an operator written in a loop
/// condition, an `if` condition, a body or a `case` arm belongs to the block's
/// own cond list. Both counters only recognize a *command-position* word — a
/// keyword-shaped argument (`echo if && echo b`) is word data, as bash and the
/// segment scanner require. Returns the operator's first byte (or `line.len()`)
/// and whether it is `||`.
pub(crate) fn next_boundary(line: &[u8], mut i: usize, state: &mut ScanState) -> (usize, bool) {
    let mut run_start = i;
    let mut group_depth: i32 = 0;
    // The block rule is off for a text that carries `<<` operators: a block's
    // condition bodies travel *inside* its span, and the `&&` written in such a
    // body line must keep splitting the part
    // (`tests/heredoc.rs::block_heredoc_cond_run_and_and_in_body_splits_part`).
    let block_rule = heredoc::operator_count(line, i, line.len()) == 0;
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
            if block_rule && state.at_word_start() {
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
                && let Some(resume) = heredoc::skip(line, run_start, i)
            {
                i = resume;
                run_start = i;
                continue;
            }
            if b == b'\n' || b == b';' {
                i += 1;
                run_start = i;
                continue;
            }
        }
        i = state.advance(line, i);
    }
    (line.len(), false)
}
