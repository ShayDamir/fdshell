//! The byte-level `<<` operator scan for a run, shared by the skip helpers
//! and the line-body extraction. The walk yields operator **sites**; the
//! delimiter is read from a site on demand, so the operator *count* never
//! depends on delimiter validity (see [`operator_count`]).

use crate::scan::ScanState;

use super::io::{io_fd, io_prefix, is_chevron, word_start};
use super::lines::delimiter_span;
use super::op::Operator;
use alloc::vec::Vec;
use pipe::precedes_pipe;

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
/// A `<<` counts as an operator when it **begins a word** of the run: at the
/// run start (the tokenizer's word 0 of the text it is handed — a
/// redirections-only statement is `simple_command = REDIRECT…` with no command
/// word, task #345) or right after a tokenizer word-break byte: whitespace,
/// `;`, a newline, a top-level `)`, and a pipeline `|` (the ONE rule
/// `parse::redirect::pipeline_pipe` decides whether a `|` byte is the stage
/// boundary or the `>|` clobber's operator byte). A `<<` glued inside a word
/// (`x<<EOF`, `><<EOF`, `>x<<EOF`) is word data at both levels. The io-number
/// form (`3<<T`, `0<<-T`) is read through the shared `io::io_prefix`, so its
/// digits begin the operator's word and the operator carries their fd;
/// `io::span_operator` is the token-level half of that same rule. LESSONS: the
/// byte-level and token-level counts must agree, per part.
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
        // The io-number form's word starts at its digits, so the word gate is
        // evaluated at the operator's own word start `run`: the operator's
        // prefix is never its own preceding token, and `io_prefix`'s word rule
        // is the narrow io one on purpose — on the physical line the glued
        // `cat&&3<<T` counts 0 (the `&` is word content). The cond splitter's
        // part window is the measured exception: it opens on the operator, so
        // the window-relative `run == from` arm counts 1 and the part asks for
        // a body the line's list never claimed — the false `missing
        // terminating delimiter` signature, pinned by
        // `tests/redirect.rs::a_heredoc_word_zero_keeps_its_owner_guards`
        // (owner: the glued-pair family).
        let io = io_prefix(line, i, &state);
        let run = io.map_or(i, |(run, _)| run);
        if bare && opens_word(line, from, run, &state) && is_chevron(line, i) {
            let fd = io.and_then(|(_, digits)| io_fd(digits)).unwrap_or(0);
            let dash = line.get(i + 2) == Some(&b'-');
            found.push(Site { op: i, dash, fd });
            let tail = if dash { i + 3 } else { i + 2 };
            let next = delimiter_span(line, tail, to).map_or(tail, |(next, _)| next);
            while i < next {
                i = state.advance(line, i);
            }
            continue;
        }
        i = state.advance(line, i);
    }
    found
}

/// The `<<` of a word that begins at `run` **begins a word of the run**: the
/// window start, a tokenizer word-break byte (`io::word_start` — whitespace,
/// `;`, a newline, a top-level `)`), or a pipeline `|` (the `>|` clobber's `|`
/// is operator content, never a break: `> <<EOF` breaks, `>|<<EOF` does not).
/// The byte mirror of `parse::heredoc::is_operator`, which reads the shape of
/// the token and needs no predecessor at all (task #345). The break set is the
/// tokenizer's separator arm, derived from `parse::token::step` and never
/// invented here: `&`, `(`, `<`, `>` are **word bytes** for the tokenizer, so
/// the physical line counts the glued `true&&<<B` / `cat&&3<<T` as word data —
/// while a cond part window opened on the operator counts 1 through the
/// window-relative `run == from` arm (the measured count mismatch: false
/// `missing terminating delimiter`, rc 1, bash rc 0), pinned by
/// `tests/redirect.rs::a_heredoc_word_zero_keeps_its_owner_guards`; the
/// divergence owner is the glued-pair family.
fn opens_word(line: &[u8], from: usize, run: usize, state: &ScanState) -> bool {
    run == from || word_start(line, run, state) || precedes_pipe(line, run)
}

#[cfg(test)]
mod tests;
