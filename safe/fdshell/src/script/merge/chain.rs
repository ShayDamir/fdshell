//! The chain the runner folds: one statement out of the segments a line-ending
//! `&&`/`||` continues (task #492).
//!
//! The chain is written as the **same-line spelling** — the links on one line,
//! separated by spaces — because the parse layer reads a newline as a `;` token
//! (LESSONS: "A `;` token is a newline in disguise"): an operator left at the end
//! of a line is no operator token for it, so a chain written with the physical
//! lines' newlines splits back into separate statements and loses every operator
//! but the first. The separator the merge itself writes is a space, and so is every
//! **separator** newline a link carries inside itself — the newlines the scan stepped
//! over as the operator's terminator and left on the segment as `joins`
//! (`segment/join.rs`, `segment/line.rs::run_joins`). A newline that is not a
//! separator stays a newline: it is data inside a quoted word, or the newline that
//! ends a `#` comment — that one must survive, since the parse layer reads the
//! comment's text only up to it and a space would let the comment swallow the next
//! link. The cost of that exception is visible and pinned: a chain with a comment on one of
//! its middle links still splits at the comment's newline
//! (`tests/heredoc.rs::a_scan_joined_run_of_three_lines_is_written_on_one_line`), so it does
//! not produce what bash does there; closing it means discarding comment text, a rule the
//! parse layer owns, not this one.

use crate::segment::Segment;
use alloc::vec::Vec;

use crate::script::merge::Joined;

/// One statement's parts; a non-statement is no link of a chain. `joins` are the
/// offsets **into `text`** of the separator newlines the scan stepped over, in scan
/// order; `join_next` says the links below belong to the same statement.
pub(super) struct Part<'a> {
    text: &'a [u8],
    bodies: &'a Vec<Vec<u8>>,
    background: bool,
    pub(super) join_next: bool,
    joins: &'a [usize],
}

/// A segment's text, bodies, flag and join points, or `None` for anything but a
/// statement.
pub(super) fn part<'a>(segment: &'a Segment<'a>) -> Option<Part<'a>> {
    match segment {
        Segment::Statement {
            cmd,
            bodies,
            background,
            join_next,
            joins,
            ..
        } => Some(Part {
            text: cmd,
            bodies,
            background: *background,
            join_next: *join_next,
            joins,
        }),
        _ => None,
    }
}

/// The chain as one statement: the texts in scan order, on one line as the
/// same-line spelling would write them, the bodies in operator order, and the
/// last part's flag.
pub(super) fn concatenate(chain: &[Part<'_>]) -> Joined {
    let mut text = Vec::new();
    let mut bodies = Vec::new();
    for (i, link) in chain.iter().enumerate() {
        // The one line the chain stands for: a space, never the newline the
        // physical lines had — the parse layer reads a newline as a `;` token
        // and would split the chain back into statements (task #492).
        if i > 0 {
            text.push(b' ');
        }
        write_on_one_line(&mut text, link);
        bodies.extend(link.bodies.iter().cloned());
    }
    let background = chain.last().map(|p| p.background).unwrap_or(false);
    Joined {
        text,
        bodies,
        background,
    }
}

/// One link's text, with its separator newlines written as spaces (task #492).
/// Every other byte, including a newline that is data, is copied as it stands.
fn write_on_one_line(text: &mut Vec<u8>, link: &Part<'_>) {
    let base = text.len();
    text.extend_from_slice(link.text);
    for rel in link.joins {
        let at = base + rel;
        if let Some(slot) = text.get_mut(at) {
            *slot = b' ';
        }
    }
}
