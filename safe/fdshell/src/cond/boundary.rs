//! `&&` / `||` boundary scanning for cond lists, skipping heredoc bodies so
//! a multi-line heredoc part stays one part and `&&` / `||` in its body do
//! not split it. Inside a keyword block the block's command text is opaque,
//! while the heredoc **body** regions it carries inline stay scanned
//! (`boundary/body.rs`). The group counter recognizes a function
//! definition's body opener as well as the plain group opener, so a body's
//! operators stay body data of the definition ([`opens_group`], task #235).
//! In-block keyword scoring follows bash's clause rule: the scan carries a
//! command-position flag ([`Place`]) so a block keyword written as an argument
//! in a body (`echo if && echo x`) stays word data (task #236).

use crate::keywords::Place;
use crate::scan::ScanState;
use crate::segment::group::{definer_at_run, group_at_run, opens_at};

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
    // Bash recognizes a reserved word in command position only. Inside a block
    // the scan carries that rule as a flag ([`Place`], task #236): every
    // separator and every cond operator restarts it, and every word moves it
    // per [`block::word_delta`]'s transition.
    let mut place = Place::Command;
    while let Some(&b) = line.get(i) {
        if !state.in_quote && !state.in_single && state.paren_depth == 0 {
            if block_depth > 0
                && (b == b'&' && line.get(i + 1) == Some(&b'&')
                    || b == b'|' && line.get(i + 1) == Some(&b'|'))
            {
                // A cond operator starts a command, also inside a block: the
                // word behind it may be a nested block's opener.
                place = Place::Command;
            }
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
            // *argument* (`echo if && echo b`) word data at top level *and*
            // inside a block (the carried `place`).
            if state.at_word_start() {
                let (delta, next) = block::word_delta(line, i, block_depth, run_start, place);
                block_depth = block_depth.saturating_add(delta).max(0);
                place = next;
            }
            // Only a brace that opens a group counts: at a cond-list part start
            // (the run is empty, or the run ends with `&&` / `||`) or as a
            // function definition's body opener. A brace in an argument position
            // (`echo x{`, `echo {`) is word data, exactly as the segment
            // scanner's `group_at_run` rule requires.
            if opens_at(line, i) && opens_group(line, run_start, i) {
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
                        place = Place::Command;
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
                place = Place::Command;
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

/// A `{` at `brace` opens an opaque group on the run that starts at
/// `run_start`: the cond-list clause rule [`group_at_run`] (empty run, a run
/// ending with `&&` / `||`, `time [-p]` transparent), plus a function
/// definition's body opener — the definer word in front of the `{` keeps the
/// opener off the run start, and the segment scanner claims the body from the
/// definer, so the body's `&&` / `||` is body data (task #235,
/// [`definer_at_run`]).
fn opens_group(line: &[u8], run_start: usize, brace: usize) -> bool {
    group_at_run(line, run_start, brace).is_some() || definer_at_run(line, run_start, brace)
}
