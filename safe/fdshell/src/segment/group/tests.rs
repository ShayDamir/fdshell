#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

/// The group span claims the whole statement it opens: the `{` to the next
/// separator, comment, or end of input, so trailing redirections and the
/// `&&`/`||`/`|` operators belong to the group. `start` is 0 (a group at the
/// start of the line), `raw` is the line, `pre_resume` is 0.
fn span(line: &[u8], end: usize, resume: usize) {
    let (segment, got_resume, floor) = group_block(line, line, 0, false, 0);
    match segment {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => {
            assert_eq!(block_start, 0, "span of {line:?}");
            assert_eq!(end_pos, end, "span end of {line:?}");
            assert!(closed, "{line:?} must be a closed group");
        }
        Segment::Statement { .. } => panic!("{line:?} must be a block"),
    }
    assert_eq!(got_resume, resume, "resume of {line:?}");
    assert_eq!(floor, resume, "floor of {line:?}");
}

#[test]
fn group_span_covers_the_statement_after_the_closing_brace() {
    span(b"{ echo a; }", 11, 11);
    span(b"{ echo a; } > out; echo b", 17, 18);
    span(b"{ echo a; } && echo b", 21, 21);
    span(b"{ echo a; } ; echo b", 12, 13);
    span(b"{ echo a; }>out3", 16, 16);
    span(b"{ echo a; } # note", 12, 18);
    span(b"{ { echo n; }; }", 16, 16);
    // A nested group that never closes is opaque: the span runs to end of input.
    span(b"{ echo a; } && { echo b", 23, 23);
}

/// A group that never closes keeps the REPL reading continuation lines: the
/// span runs to the end of the input and reports `closed = false`.
#[test]
fn unclosed_group_is_open() {
    let (segment, resume, _) = group_block(b"{ echo a", b"{ echo a", 0, false, 0);
    match segment {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => {
            assert_eq!((block_start, end_pos), (0, 8));
            assert!(!closed);
        }
        Segment::Statement { .. } => panic!("expected an open group"),
    }
    assert_eq!(resume, 9);
}

/// A statement that precedes the group on the same line owns its own body
/// regions: the group's resume starts at its own statement end, and the floor
/// keeps the trailing flush from re-entering the here-doc body.
#[test]
fn group_resume_never_reopens_the_group_body() {
    let line = b"cat <<EOF\nhi\nEOF\n; { echo a; } > out";
    // The here-doc statement ends at 17 (just past its `EOF` delimiter line);
    // the group run starts at 18 and its `{` at 19.
    let (segment, resume, floor) = group_block(line, b" { echo a", 18, false, 17);
    match segment {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => {
            assert_eq!(block_start, 18);
            assert_eq!(end_pos, line.len());
            assert!(closed);
        }
        Segment::Statement { .. } => panic!("expected a group block"),
    }
    assert_eq!(resume, line.len());
    assert_eq!(floor, line.len());
}
