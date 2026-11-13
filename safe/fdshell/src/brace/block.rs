//! The shared `{ …; }` block scan of the brace machinery: the walk both the
//! definer/function body in `brace.rs` and the group commands in
//! `segment/group/` and `segment/block/` run (four call sites, one scanner).

// mutants: -C --lib -C --test -C group -C --test -C loops -- -E 'test(/^brace::/) | binary(group) | binary(loops)'

use crate::scan::{Boundary, Quotes, boundary, heredoc, skip_comment};

/// Scan forward from `i` (just after an opening `{`) to the matching `}`,
/// tracking quotes, backticks, `$( )` and nested braces. Updates `quotes`.
/// Returns `(closing_brace_position, block_was_closed)`.
///
/// Two callers share it: the `name() { … }` function body in `brace.rs`, and the
/// `{ …; }` group command in `segment/group.rs`.
pub(crate) fn scan_brace_block(line: &[u8], mut i: usize, quotes: &mut Quotes) -> (usize, bool) {
    let mut state = quotes.scan_state();
    let mut depth = 1;
    let mut run_start = i;
    while let Some(&b) = line.get(i) {
        let bare =
            !state.in_quote && !state.in_single && !state.in_backtick && state.paren_depth == 0;
        // A here-doc operator word is one unit — its bytes *and* its operand word
        // (`<<}`, `<< }`) — so no brace inside it moves the depth (task #432). It
        // is consulted before the brace rows, at every byte the walk stops at.
        if bare && let Some(next) = heredoc::word_with_operand_end(line, i) {
            i = next;
            continue;
        }
        if bare && b == b'{' {
            depth += 1;
            i += 1;
            continue;
        }
        if bare && b == b'}' {
            depth -= 1;
            if depth == 0 {
                break;
            }
            i += 1;
            continue;
        }
        let kind = boundary(line, i, &state);
        if kind == Boundary::Char {
            i = state.advance(line, i);
            continue;
        }
        state.word_active = false;
        if kind == Boundary::Comment {
            i = skip_comment(line, i);
        } else if let Some(resume) = heredoc::skip(line, run_start, i) {
            // A heredoc body is opaque: braces in it do not close the block.
            i = resume;
        } else {
            i += 1;
        }
        run_start = i;
    }
    *quotes = state.quotes();
    (i, depth == 0)
}
