mod block;
pub(super) mod book;
pub(crate) mod group;
mod line;

use crate::brace::scan_function_block;
use crate::keywords::{first_word_end, keyword_delta};
use crate::scan::{Boundary, boundary, skip_comment};
use alloc::vec::Vec;
use book::Book;

/// A segment of a script line extracted by the scanner.
pub(crate) enum Segment<'a> {
    /// Simple statement: the command bytes (`cmd`), its offset in the line
    /// (`off`), and its heredoc body regions (`bodies`, empty when the
    /// statement has no heredoc).
    Statement {
        cmd: &'a [u8],
        off: usize,
        bodies: Vec<(usize, usize)>,
    },
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

    while book.i <= line.len() {
        let kind = boundary(line, book.i, &book.state);
        if kind == Boundary::Char {
            if !in_block && book.state.at_word_start() && group::opens_at(line, book.i) {
                group::opener::scan(line, &mut book);
            } else {
                book.i = book.state.advance(line, book.i);
            }
            continue;
        }
        book.state.word_active = false;

        let raw = line.get(book.start..book.i).unwrap_or(b"");
        let part = raw.trim_ascii();
        let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
        let word_end = first_word_end(part, book.start + lead);

        if !in_block
            && !part.is_empty()
            && (keyword_delta(part, line, word_end) == Some(1)
                || scan_function_block(line, part, book.start, book.state.quotes()).is_some())
        {
            block::open(line, raw, &mut book);
        } else {
            if !part.is_empty() {
                book.runs.push((part, book.start + lead));
            }
            if line.get(book.i) == Some(&b'\n') || book.i == line.len() {
                book.i = line::flush_line(
                    &mut book.segments,
                    line,
                    book.line_start,
                    book.i,
                    &mut book.runs,
                )
                .max(book.body_floor);
                book.line_start = book.i;
            }
            if kind == Boundary::Comment {
                book.i = skip_comment(line, book.i);
                book.start = book.i;
                // The comment ends the physical line: the next line starts at
                // `i`, so the buffered runs' line bookkeeping restarts there.
                book.line_start = book.i;
            } else {
                book.start = book.i + 1;
                book.i += 1;
            }
        }
    }
    book.segments
}

#[cfg(test)]
mod tests;
