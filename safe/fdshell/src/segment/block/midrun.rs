//! The mid-run keyword block: a block that opens a cond part after `&&` / `||`
//! is skipped **whole inside the run**, the rule
//! `segment/group/span.rs::skip_group` records for a `{ …; }` cond part ("A group
//! after `&&` is a cond part: skip its span, never flush the run before it").
//! The run keeps the block's bytes, none of the block's own `;` is a top-level
//! separator, and the cond layer splits the operators with its block-opacity
//! rule (`cond/boundary/block.rs`).

use super::length::{list_seed, opener_closer, opener_len};
use crate::brace::scan_brace_block;
use crate::comment::scan_block;
use crate::keywords::{function_def_name, keyword_delta, time_prefix_len};
use crate::scan::Quotes;
use crate::segment::book::Book;
use crate::segment::group;

/// Skip the keyword block (`if`/`for`/`while`/`until`/`case`/`wait`) or the
/// `name() { … }` definition that opens a cond part at the word start `book.i`:
/// `Some(resume)` with the cursor moved past the block's span, `None` when the
/// word opens no block (the caller advances over it as word data). An unclosed
/// span also opens the book's mid-run flag, the REPL's continuation signal
/// (task #294).
///
/// The caller owns the three-way command-position split ([`group::group_at_run`],
/// the same rule `group::opener::scan` applies to a `{`): a *run-start* keyword
/// (`Some(true)`) belongs to [`super::open`] (the block segment with its heredoc
/// region modes), a cond part mid-run (`Some(false)`) is this span, and a
/// keyword-shaped *argument* (`None`) is word data. A brace word never comes
/// here: the scanner tests `group::opens_at` first, and [`opener_len`] answers
/// `None` for it, so `{ …; }` stays owned by `segment/group.rs`.
///
/// The span end is the separator that closes the closing keyword's run: the
/// block's bytes stay in the run, a trailing operator written before that
/// separator (`done && echo y`) stays with them for the cond layer to
/// re-split, and the `;` after the closer ends the statement as usual. An
/// unclosed span resumes at the end of input, and every resume is strictly
/// past the word start, so the scan never stalls on `true && while`.
pub(in crate::segment) fn skip(line: &[u8], book: &mut Book<'_>) -> Option<usize> {
    if group::group_at_run(line, book.start, book.i) != Some(false) {
        return None;
    }
    let tail = line.get(book.i..).unwrap_or(b"");
    let mut quotes = book.state.quotes();
    let (resume, closed) = span_end(line, tail, book.i, &mut quotes)?;
    book.state.set_quotes(quotes);
    book.state.word_active = false;
    book.midrun_open |= !closed;
    book.i = resume;
    Some(resume)
}

/// The exclusive end of the block span opening at the word start `i` of `tail`,
/// `None` when the word opens no block. A keyword block is seeded past the
/// `time [-p]` prefix run and the keyword itself (the same seed
/// `keyword_block` computes) and scanned to depth 0: the span ends at the
/// separator that closes the closing keyword's run (`scan_block`'s `start`),
/// so a `;` after the closer still ends the statement and everything written
/// between the closer and that separator (`done && echo y`) stays in the run
/// for the cond layer to re-split. A function definition is scanned to its
/// closing brace, carrying the quote state out of the body exactly like
/// `skip_group`. An unclosed span ends at the end of input.
///
/// The stage arm [`super::stage`] reuses this span rule verbatim: only the
/// command-position gate in front of it differs between the two arms.
///
/// The keyword branch is gated on the **shared run-start scorer**
/// [`keyword_delta`] `== Some(1)` — `opener_len` only measures the word. The
/// scorer is what tells a block-opening `wait` from the POSIX builtin
/// (`keyword_delta` scores the builtin `0`: no pattern keyword behind it), and
/// the cond layer re-splits only `&&`/`||`, never a `;`: a span over the
/// builtin would swallow every statement after the first `;` of the run
/// (`true && wait; echo tail`). A word the scorer rejects stays word data, so
/// the caller advances over it and the run splits at its `;` as before.
///
/// The pair is (resume, closed): the caller records a `false` closedness as the
/// book's mid-run flag, which is what makes the REPL keep reading a `true &&
/// while :; do echo hi` buffer (task #294).
pub(super) fn span_end(
    line: &[u8],
    tail: &[u8],
    i: usize,
    quotes: &mut Quotes,
) -> Option<(usize, bool)> {
    let prefix = time_prefix_len(tail).unwrap_or(0);
    let keyword = tail.get(prefix..).unwrap_or(b"");
    if let Some(kw_len) = opener_len(keyword)
        && keyword_delta(keyword, line, i + prefix + kw_len) == Some(1)
    {
        let after_kw = i + prefix + kw_len;
        let mut start = after_kw;
        // The same word-list seed and the same opener-closer seed
        // [`super::keyword::keyword_block`] gives the run-start span, so a `for`
        // list word never ends this span and a closer of another kind never ends
        // it either (tasks #306, #309).
        let (_end, closed) = scan_block(
            line,
            after_kw,
            quotes,
            &mut start,
            opener_closer(keyword, line, after_kw),
            list_seed(kw_len),
        );
        let resume = if closed {
            start.saturating_sub(1)
        } else {
            line.len()
        };
        return Some((resume, closed));
    }
    function_def_name(tail, line, i)?;
    let brace = tail.iter().position(|&b| b == b'{')?;
    let (close, closed) = scan_brace_block(line, i + brace + 1, quotes);
    Some((if closed { close + 1 } else { line.len() }, closed))
}

#[cfg(test)]
mod tests;
