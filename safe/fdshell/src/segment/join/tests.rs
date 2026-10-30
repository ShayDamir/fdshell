//! Unit pins of the scan's join decision (task #492): which boundary is the
//! operator's terminator, and which arm of the decision answers it.

#![allow(clippy::unwrap_used)]
use super::{Decision, decision};
use crate::scan::Boundary;
use crate::segment::book::Book;

/// The decision at the boundary `kind` standing on `at` in `line`.
fn decided(line: &[u8], at: usize, kind: &Boundary) -> Decision {
    let mut book = Book::new();
    book.i = at;
    decision(line, &book, kind)
}

/// The index the boundary loop stands on for the row's boundary: the newline it
/// is reading, or the `#` it opened on.
fn first(line: &[u8], kind: &Boundary) -> usize {
    let needle = if *kind == Boundary::Comment {
        b'#'
    } else {
        b'\n'
    };
    line.iter().position(|&b| b == needle).unwrap_or(line.len())
}

/// A newline behind a top-level `&&`/`||` is the operator's terminator (§3), and
/// so is the newline that ends a word-start comment (§4.4); `;` and end of input
/// are not join points, and neither is a line ending on a word.
#[test]
fn only_the_operators_terminator_joins() {
    for (line, kind, want_join) in [
        (b"echo a &&\necho b".as_slice(), Boundary::Separator, true),
        (b"echo a ||\necho b".as_slice(), Boundary::Separator, true),
        (b"echo a && #c\necho b".as_slice(), Boundary::Comment, true),
        (b"echo a && ; echo b".as_slice(), Boundary::Separator, false),
        (b"echo a &&".as_slice(), Boundary::Separator, false),
        (b"echo a\necho b".as_slice(), Boundary::Separator, false),
        (
            b"echo a \"&&\"\necho b".as_slice(),
            Boundary::Separator,
            false,
        ),
        (
            b"echo a \\&&\necho b".as_slice(),
            Boundary::Separator,
            false,
        ),
        (b"echo a #c\necho b".as_slice(), Boundary::Comment, false),
    ] {
        let at = first(line, &kind);
        let joins = matches!(decided(line, at, &kind), Decision::Join(_));
        assert_eq!(joins, want_join, "{line:?}");
    }
}

/// The exception (§4.3/§4.5): a run whose own command line wrote a `<<` keeps
/// its own segment and body, so the decision hands the concatenation to the
/// runner instead of joining the run.
#[test]
fn a_run_that_wrote_a_heredoc_goes_to_the_runner() {
    let line = b"echo a <<B &&\nbody\nB\necho b";
    let at = first(line, &Boundary::Separator);
    assert!(matches!(
        decided(line, at, &Boundary::Separator),
        Decision::Merge
    ));
}

/// The same exception one step further: the `<<` stands in the `;`-separated run
/// **ahead** of the one that ends on the terminator, so the physical line claims a
/// region and the flush keeps its per-line segments — and the mark rides on the
/// joining run alone (`segment/line.rs`, `script/merge.rs`).
#[test]
fn a_region_claimed_by_an_earlier_run_of_the_line_goes_to_the_runner() {
    //   c a t   < < B  ;  e c h o   a  && \n b o d y \n B \n e c h o   t a i l
    //   0          8  10        15 17 18  19
    let line = b"cat <<B ; echo a &&\nbody\nB\necho tail";
    let mut book = Book::new();
    book.line_start = 0;
    book.start = 10;
    book.i = first(line, &Boundary::Separator);
    assert!(matches!(
        decision(line, &book, &Boundary::Separator),
        Decision::Merge
    ));
}
