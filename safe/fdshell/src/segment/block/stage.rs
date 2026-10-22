//! The **pipeline-stage** keyword block: a block that opens right behind a `|`
//! is skipped **whole inside the run**, exactly as [`super::midrun`] skips a
//! cond-part block — the run keeps the block's bytes, none of the block's own
//! `;` is a top-level separator, and the pipeline parser splits the stages
//! (task #292). The arm is the fourth of the command-position classification
//! ([`group::stage_at_run`]); the segment scanner reaches it only where the
//! cond-part gate answers `None`, so the two spans never overlap.
//!
//! Two asymmetries vs. the cond arm, both measured against bash 5.3.9:
//!
//! - a `{ …; }` group at this position stays a **word** (tasks #186/#190:
//!   `echo a | { echo b; }` is a parse error), while `&& { …; }` is an opaque
//!   cond part — the brace rule is [`group::group_at_run`], which this arm
//!   never consults;
//! - a `time [-p]` behind the `|` is **not** a transparent prefix of the
//!   stage's keyword: the stage's word 0 is `time` (bash rejects
//!   `echo a | time while :; do :; done` as a syntax error, rc 2), while
//!   behind `&&` a prefix is transparent (`true && time while …` runs).

use crate::segment::book::Book;
use crate::segment::group;

use super::midrun::span_end;

/// Skip the keyword block (`if`/`for`/`while`/`until`/`case`/`wait`) or the
/// `name() { … }` definition that opens a pipeline stage at the word start
/// `book.i`: `Some(resume)` with the cursor moved past the block's span,
/// `None` when the word opens no block (the caller advances over it as word
/// data, and a keyword-shaped word of a plain stage splits its `;` as before).
///
/// The span rule is [`span_end`]'s, verbatim: the span ends at the separator
/// that closes the closing keyword's run, so the `;` after the closer ends the
/// statement, everything written between the closer and that separator
/// (`done | cat`, `done && echo y`) stays in the run for the pipeline and cond
/// layers to split, and an unclosed span resumes at the end of input.
pub(in crate::segment) fn skip(line: &[u8], book: &mut Book<'_>) -> Option<usize> {
    if !group::stage_at_run(line, book.start, book.i) {
        return None;
    }
    let tail = line.get(book.i..).unwrap_or(b"");
    let mut quotes = book.state.quotes();
    let resume = span_end(line, tail, book.i, &mut quotes)?;
    book.state.set_quotes(quotes);
    book.state.word_active = false;
    book.i = resume;
    Some(resume)
}

#[cfg(test)]
mod tests;
