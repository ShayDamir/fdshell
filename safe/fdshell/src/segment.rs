mod bg;
mod block;
pub(super) mod book;
pub(crate) mod group;
mod join;
mod line;

use crate::scan::{Boundary, boundary, skip_comment};
use alloc::vec::Vec;
use book::{Book, Run};

/// A segment of a script line extracted by the scanner.
pub(crate) enum Segment<'a> {
    /// Simple statement: the command bytes (`cmd`), its offset in the line
    /// (`off`), its heredoc body bytes (`bodies`, empty when the statement
    /// has no heredoc), the plain-`&` flag marking its **last** run to
    /// run asynchronously (`background`, task #326), and the flag saying one
    /// of its runs opens a mid-run construct whose closer never arrived
    /// (`midrun_open`, task #294), and whether its text ends on the operator's
    /// terminator so that the segments below it belong to the same statement
    /// (`join_next`, task #492).
    Statement {
        cmd: &'a [u8],
        off: usize,
        bodies: Vec<Vec<u8>>,
        background: bool,
        /// A mid-run construct of this run (`&& { …`, `&& while … do`,
        /// `| while … do`, `&& f() {`) has no closer: the scan skipped its
        /// span to the end of input, so this is the REPL's continuation
        /// signal (task #294) and inert for the script runner, whose parse
        /// layer owns the missing-closer diagnostic.
        midrun_open: bool,
        /// The run's own command line wrote a `<<` and ended on the operator's
        /// terminator (POSIX #2.9), so its body is a region on the lines below
        /// and the runner concatenates it with the segments it continues onto
        /// (`script/merge.rs`, task #492).
        join_next: bool,
    },
    /// A plain `&` whose split is invalid (nothing to terminate, `;`-separated
    /// target, or a `;`/`|`/`&` peek behind it — bash's "syntax error, nothing
    /// runs" family). `run_script` bails on it **before executing any**
    /// segment; it is never executed itself.
    AsyncInvalid,
    /// Block (`if … fi`, `while … done`, `case … esac`, `name() { … }`,
    /// `{ …; }`) spanning from `block_start` to `end_pos`.
    Block {
        block_start: usize,
        end_pos: usize,
        /// Whether the closing keyword was found.
        closed: bool,
        /// The block's own heredoc body bytes, appended to its text; empty
        /// when the block carries no `<<` operator, or when its bodies are
        /// interleaved with its lines (extension mode, see
        /// [`block::keyword::keyword_block`]).
        bodies: Vec<Vec<u8>>,
    },
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
            Boundary::Char => {
                let opener = !in_block && book.state.at_word_start();
                if opener && group::opens_at(line, book.i) {
                    group::opener::scan(line, &mut book);
                } else if opener && block::midrun::skip(line, &mut book).is_some() {
                    // A keyword block that opens a cond part mid-run: `skip` moved
                    // the cursor past its whole span, so its `;` never splits the
                    // run and the cond layer splits the operators.
                } else if opener && block::stage::skip(line, &mut book).is_some() {
                    // A keyword block that opens a pipeline stage mid-run: `skip`
                    // moved the cursor past its whole span, so its `;` never splits
                    // the run and the pipeline parser splits the stages.
                } else {
                    book.i = book.state.advance(line, book.i);
                }
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
        // A physical line that claims a body region is the exception: its body is
        // a region on the lines below, and the region text cannot be both inline in
        // the run's bytes and attached in operator order, so the physical lines keep
        // their own segments (each with its own bodies) and `script/merge.rs` joins
        // them in the runner. The mark rides on the run the decision was made for —
        // the flush's last run — never on a `;`-separated run flushed beside it.
        match join::decision(line, &book, &kind) {
            join::Decision::Join(nl) => {
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
            let sep = book.i;
            if line.get(book.i) == Some(&b'\n') || book.i == line.len() {
                deferred = line.get(book.i) == Some(&b'\n') && bg::async_ahead(line, book.i);
                book.i = line::end_line(&mut book, line, deferred).max(book.body_floor);
            }
            book.resume_past(sep, kind, deferred, line);
        }
    }
    book.segments
}

#[cfg(test)]
mod tests;
