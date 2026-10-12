//! The `{` branch of the segment scan: what a `{` written at a word start does
//! to the scan.

use crate::segment::book::Book;

/// Take the group branch of the segment scan at the `{` the scanner found at a
/// word start: a group at a **run start** is a block (the run's statements are
/// flushed, the group's span is pushed, the cursor is rewound past it with the
/// line's resume floor); a group at a **cond-part start mid-run** is skipped
/// whole so the body's `;` is not a top-level separator (the run keeps the
/// group's text and the cond layer splits the operator); a brace in an
/// **argument position** is word data (advance one byte).
pub(crate) fn scan<'a>(line: &'a [u8], book: &mut Book<'a>) {
    let (start, i) = (book.start, book.i);
    match super::group_at_run(line, start, i) {
        Some(true) => super::open(line, book),
        Some(false) => book.i = super::skip_group(line, i, &mut book.state),
        None => book.i = book.state.advance(line, i),
    }
}
