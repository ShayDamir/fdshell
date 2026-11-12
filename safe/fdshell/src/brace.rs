// mutants: -C --lib -C --test -C group -C --test -C loops -- -E 'test(/^brace::/) | binary(group) | binary(loops)'

use crate::keywords::{function_def_name, opens_a_block};
use crate::scan::{Boundary, Quotes, boundary, heredoc, skip_comment};

/// If `part` opens a `name() { … }` block, return the exclusive end position
/// and whether the block was closed. `quotes` is the quote state at `start`,
/// and `start` is `part`'s first byte in `line` (the definer gate's contract).
pub(crate) fn scan_function_block(
    line: &[u8],
    part: &[u8],
    start: usize,
    quotes: Quotes,
) -> Option<(usize, bool)> {
    function_def_name(part, line, start)?;
    Some(brace_block_behind_a_head(line, start, quotes))
}

/// If `part` is an arithmetic-`for` head with a `{ …; }` body written behind it
/// (task #394), return that block — the definer arm's shape, because the head plays
/// the definer's role: the run it is written alone in opens the block, and the parse
/// layer sees the head and its body as one statement. A head with a `do … done`
/// body behind it is not this arm's: the keyword row owns it.
pub(crate) fn scan_arith_for_block(
    line: &[u8],
    part: &[u8],
    start: usize,
    quotes: Quotes,
) -> Option<(usize, bool)> {
    opens_a_block(part, line, start).then(|| brace_block_behind_a_head(line, start, quotes))
}

/// The `{ …; }` block written behind a head word, at or after the byte its own
/// text starts at ([`scan_brace_block`]'s scan) — one body for the definer arm and
/// the arithmetic-`for` arm, so the two cannot drift.
fn brace_block_behind_a_head(line: &[u8], start: usize, quotes: Quotes) -> (usize, bool) {
    let brace = brace_opener(line, start, quotes);
    let mut quotes = quotes;
    let (close_pos, closed) = scan_brace_block(line, brace + 1, &mut quotes);
    let end = line.len().min(close_pos + 1);
    (end, closed)
}

/// The first `{` at or after `start`: the definer arm's naive byte search, with
/// the one arm comments are opaque to byte scanners (task #518) — the shared
/// walk's gate and advance, so a comment row is skipped whole and a brace written
/// in comment text is not the body's opener. Bytes that are not a gated comment
/// row land exactly where the naive search landed. The end of the walk is the
/// range's own `is_empty`, not a `<` bound: the bound only re-probed a position no
/// search in the body can match, so it was a shield for a `<`→`<=` mutant no test
/// can see (CATCHING_MUTANTS rule 18).
fn brace_opener(line: &[u8], start: usize, quotes: Quotes) -> usize {
    let mut state = quotes.scan_state();
    let mut i = start;
    loop {
        if (i..line.len()).is_empty() {
            break;
        }
        if line.get(i) == Some(&b'{') {
            return i;
        }
        if boundary(line, i, &state) == Boundary::Comment {
            i = skip_comment(line, i);
            continue;
        }
        i = state.advance(line, i);
    }
    line.len()
}

/// Scan forward from `i` (just after an opening `{`) to the matching `}`,
/// tracking quotes, backticks, `$( )` and nested braces. Updates `quotes`.
/// Returns `(closing_brace_position, block_was_closed)`.
///
/// Two callers share it: the `name() { … }` function body above, and the
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

#[cfg(test)]
mod tests;
