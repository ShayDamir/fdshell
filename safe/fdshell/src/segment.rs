// mutants: -C --lib -- -E 'test(/^segment::/)'

mod bg;
mod block;
pub(super) mod book;
pub(crate) mod group;
mod join;
mod line;
mod types;

pub(crate) use types::Segment;

use crate::keywords::{Decision, Pairing, keyword_len};
use crate::scan::{Boundary, boundary, heredoc, skip_comment};
use alloc::vec::Vec;
use book::{Book, Run};

/// The [`Decision`] of task #436 for the run's first word — the top level's
/// command position, so [`Pairing`] is empty and any closing keyword word closes
/// no open block of its kind. The word is the run's first `keyword_len` bytes, so
/// the **one** recognition table answers here exactly as it does for the scorers
/// (the quote gate, the `time [-p]` strip, the one word-break rule); `off` is the
/// word's own offset in `line`, which is where the report draws the caret. An
/// ordinary word (and an empty run) is no closer, so the answer is `None`.
fn is_stray_closer(line: &[u8], word: &[u8], off: usize) -> bool {
    Pairing::new().decision(word, line, off + keyword_len(word)) == Some(Decision::Stray)
}

/// Scan a script line and return segments with their positions.
///
/// When `in_block` is true, block-opening words (`if`/`for`/`while`/`case`,
/// a `name() {` definition, a `{` group) are treated as regular statement
/// content rather than new blocks. This prevents re-detecting nested blocks
/// inside already-scanned block bodies.
pub(crate) fn scan_segments(line: &[u8], in_block: bool) -> Vec<Segment<'_>> {
    let mut book = Book::new();
    // A newline whose next line opens with `&` defers this line's flush.
    let mut deferred = false;

    while book.i <= line.len() {
        let kind = boundary(line, book.i, &book.state);
        match kind {
            // The word-start arm: the opener owners are asked in turn, each
            // moving the cursor past its whole span, and a run that opens
            // nothing is the plain advance. The plain-`&` arm below is its twin.
            Boundary::Char => {
                block::opener::open_at_word(line, &mut book, in_block);
                continue;
            }
            // `mark` owns the cursor for the whole split (it flushes the runs
            // the `&` terminates), so the separator machinery must not touch
            // the index it left behind. The same opener gate the separator arm
            // applies decides which branch this is: a run that opens a block is
            // opened as a block, so the `&` stays inside the block's text.
            Boundary::Async => {
                block::opener::open_at_amp(line, &mut book, in_block);
                continue;
            }
            _ => {}
        }
        book.state.word_active = false;

        // POSIX #2.9's AND-IF-TERMINATOR (task #492): a physical line that ends
        // on a top-level `&&`/`||` does not end the cond list — the operator and
        // its newline are one token, so neither the run nor the line is flushed
        // here and the next physical line joins the same run. The word-state
        // reset above still applies (the next line starts a new word), and
        // `book.start`/`line_start` stay, so the flush's region window covers
        // every physical line the run spans. A word-start `#` reads the same
        // rule with its own window: comment text is not a token (bash's lexer
        // discards it), so the predicate is read on the text before the `#` and
        // the join point is the newline that ends the comment. `;`, end of
        // input, a lone `|` and the plain `&` are not join points.
        //
        // A **separator** join point is recorded on the book (`sep_joins`) for the run
        // that spans it, so the runner — which writes a merged chain as one line — knows
        // which of that run's newlines are separators. A comment's terminating newline is
        // not recorded: the comment's text is data up to that newline, and the parse layer
        // needs the newline to end it.
        //
        // A physical line that claims a body region is the exception: its body is
        // a region on the lines below, and the region text cannot be both inline in
        // the run's bytes and attached in operator order, so the physical lines keep
        // their own segments (each with its own bodies) and `script/merge.rs` joins
        // them in the runner. The mark rides on the run the decision was made for —
        // the flush's last run — never on a `;`-separated run flushed beside it.
        match join::decision(line, &book, &kind) {
            join::Decision::Join(nl) => {
                if kind == Boundary::Separator {
                    book.sep_joins.push(nl);
                }
                book.i = skip_comment(line, nl);
                continue;
            }
            join::Decision::Merge => book.join_next = true,
            join::Decision::Flush => {}
        }

        let raw = line.get(book.start..book.i).unwrap_or(b"");
        let part = raw.trim_ascii();
        let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();

        if !in_block && block::opener::opens_run(line, raw, book.start, book.state.quotes()) {
            block::open(line, raw, &mut book);
        } else {
            if !part.is_empty() {
                book.runs.push(Run {
                    bytes: part,
                    off: book.start + lead,
                    background: false,
                });
                book.sep_since_run = false;
            }
            // A `;;` pair here is the stray of task #423: the run was not
            // claimed as a block, so this separator is not a `case` clause's
            // terminator, and the mark is the preflight's business (bash runs
            // nothing). The run path is the only place it can arise.
            if kind == Boundary::SemiSemi {
                book.segments.push(Segment::UnexpectedSemiSemi);
            }
            // Task #436's decision, on the run's **first** word — the top level's
            // command position, with this scan's stack empty: a closing keyword
            // word here closes no open block of its kind. A run that was claimed
            // as a block reports through its own span walk above. The word's own
            // offset is computed once: the ask and the report read the same number.
            let word_off = book.start + lead;
            if !in_block && is_stray_closer(line, part, word_off) {
                book.segments.push(Segment::StrayCloser { off: word_off });
            }
            let sep = book.i;
            if line.get(book.i) == Some(&b'\n') || book.i == line.len() {
                // The deferral is for lines that claim **nothing** (POSIX_COMPATIBILITY,
                // "Line deferral"): a line that claims a body region ends itself with
                // that region attached, and the `&` line below it is region data — bash
                // never sees an operator there. The deferral decision asks the one
                // definition of an opaque body for the physical line, and survives only
                // when its answer is `None`.
                deferred = line.get(book.i) == Some(&b'\n')
                    && bg::async_ahead(line, book.i)
                    && heredoc::skip(line, book.line_start, book.i).is_none();
                book.i = line::end_line(&mut book, line, deferred).max(book.body_floor);
            }
            book.resume_past(sep, kind, deferred, line);
        }
    }
    book.segments
}

#[cfg(test)]
mod tests;
