mod bg;
mod block;
pub(super) mod book;
pub(crate) mod group;
mod line;

use crate::brace::scan_function_block;
use crate::keywords::{first_word_end, keyword_delta};
use crate::scan::{Boundary, boundary};
use alloc::vec::Vec;
use book::{Book, Run};

/// A segment of a script line extracted by the scanner.
pub(crate) enum Segment<'a> {
    /// Simple statement: the command bytes (`cmd`), its offset in the line
    /// (`off`), its heredoc body regions (`bodies`, empty when the statement
    /// has no heredoc), and the plain-`&` flag marking its **last** run to
    /// run asynchronously (`background`, task #326).
    Statement {
        cmd: &'a [u8],
        off: usize,
        bodies: Vec<(usize, usize)>,
        background: bool,
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
        /// The block's own heredoc body regions, appended to its text; empty
        /// when the block carries no `<<` operator, or when its bodies are
        /// interleaved with its lines (extension mode, see
        /// [`block::keyword::keyword_block`]).
        bodies: Vec<(usize, usize)>,
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
                } else {
                    book.i = book.state.advance(line, book.i);
                }
                continue;
            }
            // `mark` owns the cursor for the whole split (it flushes the runs
            // the `&` terminates), so the separator machinery must not touch
            // the index it left behind.
            Boundary::Async => {
                bg::mark(line, &mut book);
                continue;
            }
            _ => {}
        }
        book.state.word_active = false;

        let raw = line.get(book.start..book.i).unwrap_or(b"");
        let part = raw.trim_ascii();
        let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
        let word_end = first_word_end(part, book.start + lead);

        if !in_block
            && !part.is_empty()
            && (keyword_delta(part, line, word_end) == Some(1)
                || scan_function_block(line, part, book.start + lead, book.state.quotes())
                    .is_some())
        {
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
