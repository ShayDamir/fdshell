//! The run-opening gate shared by the segment scanner's two boundaries: a run
//! opens a keyword block or a `name() { … }` definition. The separator arm and
//! the plain-`&` arm read it identically, so a `&` that is a run's first
//! boundary cannot flush an incomplete block as a statement (task #354).

use crate::brace::scan_function_block;
use crate::keywords::{first_word_end, keyword_delta, leading_ws};
use crate::scan::Quotes;
use crate::segment::bg;
use crate::segment::book::Book;

/// `true` when the run `raw` starting at `start` opens a block: a block-opening
/// keyword word (`keyword_delta == Some(1)`, which scores a quoted keyword word
/// `None` — a quoted `case`/`if` is a command word, task #247) or a
/// `name() { … }` definition. `quotes` is the scanner's quote state at the
/// cursor, and an empty run opens nothing.
pub(in crate::segment) fn opens_run(line: &[u8], raw: &[u8], start: usize, quotes: Quotes) -> bool {
    let part = raw.trim_ascii();
    if part.is_empty() {
        return false;
    }
    // The definer gate walks `line` from the run's first non-space byte.
    let def_start = start + leading_ws(raw);
    keyword_delta(part, line, first_word_end(part, def_start)) == Some(1)
        || scan_function_block(line, part, def_start, quotes).is_some()
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

#[cfg(test)]
mod tests;
