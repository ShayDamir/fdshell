//! The run-opening gate shared by the segment scanner's two boundaries: a run
//! opens a keyword block or a `name() { … }` definition. The separator arm and
//! the plain-`&` arm read it identically, so a `&` that is a run's first
//! boundary cannot flush an incomplete block as a statement (task #354).
//! The arm dispatches that read the gate — the word-start arm
//! ([`open_at_word`]) and the plain-`&` arm ([`open_at_amp`]) — live here too;
//! each owner they call keeps its own span rule.

// mutants: -C --lib -- -E 'test(/^segment::/)'

use super::{midrun, stage};
use crate::brace::{scan_arith_for_block, scan_function_block};
use crate::keywords::{first_word_end, keyword_delta, leading_ws};
use crate::scan::Quotes;
use crate::segment::bg;
use crate::segment::book::Book;
use crate::segment::group;

/// `true` when the run `raw` starting at `start` opens a block: a block-opening
/// keyword word (`keyword_delta == Some(1)`, which scores a quoted keyword word
/// `None` — a quoted `case`/`if` is a command word, task #247), a
/// `name() { … }` definition, or an arithmetic-`for` head written alone in the run
/// (task #394 — the head scores `None` too, so this gate is what opens its block).
/// `quotes` is the scanner's quote state at the cursor, and an empty run opens
/// nothing.
pub(in crate::segment) fn opens_run(line: &[u8], raw: &[u8], start: usize, quotes: Quotes) -> bool {
    let part = raw.trim_ascii();
    if part.is_empty() {
        return false;
    }
    // The definer gate walks `line` from the run's first non-space byte.
    let def_start = start + leading_ws(raw);
    keyword_delta(part, line, first_word_end(part, def_start)) == Some(1)
        || scan_function_block(line, part, def_start, quotes).is_some()
        || scan_arith_for_block(line, part, def_start, quotes).is_some()
}

/// The plain-`&` boundary. A run that opens a block is opened as a **block**:
/// the `&` sits inside the block's text, where the body's own scan marks the
/// statement background. A run that opens nothing is the statement split
/// ([`bg::mark`], task #326). `in_block` mirrors the separator arm's rule:
/// inside an already-scanned block a keyword word is statement content, never a
/// new block.
pub(in crate::segment) fn open_at_amp<'a>(line: &'a [u8], book: &mut Book<'a>, in_block: bool) {
    let raw = line.get(book.start..book.i).unwrap_or(b"");
    if in_block || !opens_run(line, raw, book.start, book.state.quotes()) {
        bg::mark(line, book);
        return;
    }
    // The separator arm clears the flag before opening a block and `open` does
    // not: a word left active past the span eats a `#` written behind it.
    book.state.word_active = false;
    super::open(line, raw, book);
}

/// The word-start boundary: each owner of a span that opens at a word start is
/// asked in turn, and a run that opens nothing is the scanner's plain advance.
/// `in_block` mirrors the plain-`&` arm's rule: inside an already-scanned block
/// a keyword word is statement content, never a new block.
pub(in crate::segment) fn open_at_word<'a>(line: &'a [u8], book: &mut Book<'a>, in_block: bool) {
    let opener = !in_block && book.state.at_word_start();
    if opener && group::opens_at(line, book.i) {
        group::opener::scan(line, book);
    } else if opener && midrun::skip(line, book).is_some() {
        // A keyword block that opens a cond part mid-run: `skip` moved the
        // cursor past its whole span, so its `;` never splits the run and the
        // cond layer splits the operators.
    } else if opener && stage::skip(line, book).is_some() {
        // A keyword block that opens a pipeline stage mid-run: `skip` moved the
        // cursor past its whole span, so its `;` never splits the run and the
        // pipeline parser splits the stages.
    } else {
        book.i = book.state.advance(line, book.i);
    }
}

#[cfg(test)]
mod tests;
