//! Where a physical-line end is the operator's terminator, not a separator
//! (POSIX #2.9's AND-IF-TERMINATOR / OR-IF-TERMINATOR, task #492): the decision
//! the scan loop consults at each boundary it reaches.
//!
//! The predicate is the shared state-aware walk (§3) — quotes, escapes, comments,
//! `$( )`, backticks, regions and the glued pair (#496) are read by the existing
//! scan rules, never by a byte compare (LESSONS: "A separator-shaped word needs
//! the span gate"). Only a newline is a join point: `;` and end of input are not
//! (bash rejects an operator with nothing behind it, rc 2, and task #367 owns
//! that report), and a word-start `#` is a join point only when a newline ends
//! the comment.

use crate::scan::{Boundary, comment_end, heredoc, last_op};
use crate::segment::book::Book;

/// What the boundary at `book.i` means for the run accumulated so far.
pub(super) enum Decision {
    /// Not a join point: the run is flushed here, as before.
    Flush,
    /// The operator's terminator: the run is not flushed and the next physical
    /// line joins the same run; `at` is the newline to step over.
    Join(usize),
    /// The same terminator, but the run's own command line wrote a `<<` whose
    /// body is a region on the lines below: the region text cannot be both inline
    /// in the run's bytes and attached in operator order (LESSONS: "A heredoc
    /// body is a property of the logical line, not of a contiguous span"), so
    /// every physical line keeps its own segment, each with its own bodies, and
    /// the runner concatenates them — `script/merge.rs`.
    Merge,
}

/// The decision at the boundary `kind` at `book.i`, read on the run's text up to
/// that boundary (comment text is not a token, so the comment arm reads the
/// predicate before the `#` and the join point is the newline that ends it).
pub(super) fn decision(line: &[u8], book: &Book<'_>, kind: &Boundary) -> Decision {
    let nl = match *kind {
        Boundary::Separator => (line.get(book.i) == Some(&b'\n')).then_some(book.i),
        Boundary::Comment => {
            (comment_end(line, book.i) < line.len()).then_some(comment_end(line, book.i))
        }
        _ => None,
    };
    let Some(nl) = nl else {
        return Decision::Flush;
    };
    if !last_op::ends_on_operator(line, book.i) {
        return Decision::Flush;
    }
    if heredoc::operator_count(line, book.start, book.i) == 0 {
        Decision::Join(nl)
    } else {
        Decision::Merge
    }
}

#[cfg(test)]
mod tests;
