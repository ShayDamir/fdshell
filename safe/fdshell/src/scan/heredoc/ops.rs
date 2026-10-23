//! The byte-level `<<` operator scan for a run, shared by the skip helpers
//! and the line-body extraction. The walk yields operator **sites**; the
//! delimiter is read from a site on demand, so the operator *count* never
//! depends on delimiter validity (see [`operator_count`]).

use crate::scan::ScanState;

use super::io::{io_fd, io_prefix, word_start};
use super::lines::delimiter_span;
use super::op::Operator;
use alloc::vec::Vec;
use pipe::{precedes_pipe, token_before};

mod pipe;

/// One `<<` operator site: the index of its first `<`, the `<<-` marker and the
/// io-number descriptor. Deliberately no delimiter: the count is a shape count,
/// and the delimiter's validity is the parse layer's error (`operator_at`'s
/// `InvalidRedirect`), not a counting gate.
pub(crate) struct Site {
    op: usize,
    dash: bool,
    fd: i32,
}

/// The `Operator` of every `<<` in the run, in order. `None` when a bare
/// `<<` (or `<<-`) has no delimiter word, which is what the region/skip layer
/// needs: a line whose operators it cannot read claims no body region, so the
/// parse layer reports the shape. A `#` comment (outside quotes, at a word
/// start) is skipped whole, so a `<<` in a comment is not an operator (the
/// tokenizer never yields it, and the two counts must agree).
///
/// A `<<` counts as an operator when a token precedes its word and that token
/// is not a pipeline `|` — the byte mirror of `parse::heredoc::is_operator`
/// (`i != 0 && prev != |`). A `<<` at the run start is the command word and
/// never counts. The io-number form (`3<<T`, `0<<-T`) is read through the
/// shared `io::io_prefix`, so its digits begin the operator's word and the
/// operator carries their fd; `io::span_operator` is the token-level half of
/// that same rule (LESSONS: the byte-level and token-level counts must agree).
pub(crate) fn operator_delims(line: &[u8], from: usize, to: usize) -> Option<Vec<Operator>> {
    let mut found: Vec<Operator> = Vec::new();
    for site in sites(line, from, to) {
        let (_, raw) = delimiter_span(line, site.op + 2 + usize::from(site.dash), to)?;
        found.push(Operator::new(raw, site.dash, site.fd));
    }
    Some(found)
}

/// The number of `<<` operators in the run `line[from..to]`: exactly what the
/// token layer counts (`parse::heredoc::is_operator` has no delimiter test, so
/// a bare `<<` with an invalid delimiter is counted there too). Delimiter
/// validity is the parse's business; the region layer's `operator_delims` keeps
/// its `None`, and the window-subset invariant (`regions.rs`) makes the split
/// allocation-neutral: every counting window sits inside the physical line's
/// window, whose region list is empty exactly when a delimiter is unreadable.
pub(crate) fn operator_count(line: &[u8], from: usize, to: usize) -> usize {
    sites(line, from, to).len()
}

/// The byte index of every `<<` of the run `line[from..to]`, in operator order
/// — the same sites [`operator_delims`] walks, so the list is index-aligned
/// with the line's body regions (one region per operator, `full.rs`). The
/// window is the region layer's own window, never a wider one.
pub(crate) fn operator_sites(line: &[u8], from: usize, to: usize) -> Vec<usize> {
    sites(line, from, to).into_iter().map(|s| s.op).collect()
}

/// Every `<<` site of the run `line[from..to]`, in order. The walk gates are
/// the token layer's rules (see [`operator_delims`]' doc); `delimiter_span` is
/// used only to step past the operator's word, and on `None` (a bare `<<` with
/// no readable delimiter) the site is still recorded and the walk continues at
/// the operator's own tail. `pub(crate)` for the window-blind count
/// ([`super::opaque`]), which reuses this walker unchanged.
pub(crate) fn sites(line: &[u8], from: usize, to: usize) -> Vec<Site> {
    let mut state = ScanState::new();
    let mut found: Vec<Site> = Vec::new();
    let mut seen_token = false;
    let mut i = from;
    while i < to {
        let bare =
            !state.in_quote && !state.in_single && !state.in_backtick && state.paren_depth == 0;
        if bare && !state.word_active && line.get(i) == Some(&b'#') {
            match line
                .get(i..)
                .and_then(|s| s.iter().position(|&b| b == b'\n'))
            {
                Some(p) => i = i + p + 1,
                None => return found,
            }
            continue;
        }
        // The io-number form's word starts at its digits, so both the word
        // gate and the pipeline gate are evaluated at the operator's own word
        // start `run`: the operator's prefix is never its own preceding token
        // (`is_operator`'s `i != 0`), and a `|` written right before the
        // digits is a stage boundary (`prev != |`).
        let io = io_prefix(line, i, &state);
        let run = io.map_or(i, |(run, _)| run);
        let preceded = if run == i {
            seen_token && word_start(line, i, &state)
        } else {
            token_before(line, from, run)
        };
        if bare
            && preceded
            && line.get(i) == Some(&b'<')
            && line.get(i + 1) == Some(&b'<')
            && line.get(i + 2) != Some(&b'<')
            && !precedes_pipe(line, run)
        {
            let dash = line.get(i + 2) == Some(&b'-');
            let fd = io.and_then(|(_, digits)| io_fd(digits)).unwrap_or(0);
            found.push(Site { op: i, dash, fd });
            let tail = if dash { i + 3 } else { i + 2 };
            let next = delimiter_span(line, tail, to).map_or(tail, |(next, _)| next);
            while i < next {
                i = state.advance(line, i);
            }
            continue;
        }
        // Read the byte before `advance`: the `\<`, `$(`, `((` and quoted-region
        // arms jump 2+ bytes, so the post-jump byte is not the token's first byte.
        let byte = line.get(i).copied();
        i = state.advance(line, i);
        if byte.is_some_and(pipe::is_token_byte) {
            seen_token = true;
        }
    }
    found
}

#[cfg(test)]
mod tests;
