#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::scan::Quotes;
use crate::segment::scan_segments;

/// The group span claims the whole statement it opens: the `{` to the next
/// separator, comment, or end of input, so trailing redirections and the
/// `&&`/`||`/`|` operators belong to the group. `start` is 0 (a group at the
/// start of the line), `pre_resume` is 0.
fn span(line: &[u8], end: usize, resume: usize) {
    let (segment, got_resume, floor) = group_block(line, 0, 0, Quotes::new(), 0);
    match segment {
        Segment::Block {
            block_start,
            end_pos,
            closed,
            bodies: _,
        } => {
            assert_eq!(block_start, 0, "span of {line:?}");
            assert_eq!(end_pos, end, "span end of {line:?}");
            assert!(closed, "{line:?} must be a closed group");
        }
        Segment::Statement { .. } => panic!("{line:?} must be a block"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
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
    let (segment, resume, _) = group_block(b"{ echo a", 0, 0, Quotes::new(), 0);
    match segment {
        Segment::Block {
            block_start,
            end_pos,
            closed,
            bodies: _,
        } => {
            assert_eq!((block_start, end_pos), (0, 8));
            assert!(!closed);
        }
        Segment::Statement { .. } => panic!("expected an open group"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
    assert_eq!(resume, 9);
}

/// A statement that precedes the group on the same line owns its own body
/// regions: the group's resume starts at its own statement end, and the floor
/// keeps the trailing flush from re-entering the here-doc body. `start` is the
/// group's `{` byte (19), `pre_resume` the here-doc statement's end (17).
#[test]
fn group_resume_never_reopens_the_group_body() {
    let line = b"cat <<EOF\nhi\nEOF\n; { echo a; } > out";
    let (segment, resume, floor) = group_block(line, 19, 19, Quotes::new(), 17);
    match segment {
        Segment::Block {
            block_start,
            end_pos,
            closed,
            bodies: _,
        } => {
            assert_eq!(block_start, 19);
            assert_eq!(end_pos, line.len());
            assert!(closed);
        }
        Segment::Statement { .. } => panic!("expected a group block"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
    assert_eq!(resume, line.len());
    assert_eq!(floor, line.len());
}

/// The byte just after a group's closing brace is inside the span: a body that
/// ends with its required `;` (`{ echo a;}`) keeps that `;` in the span, and the
/// statement-end scan starts *past* the brace, never at the body's `;`.
#[test]
fn span_ends_past_the_closing_brace() {
    span(b"{ echo a;}", 10, 10);
    span(b"{ echo a; }", 11, 11);
    span(b"{ echo a; } ; echo b", 12, 13);
}

/// A second group in the same statement is opaque: the span resumes just past
/// its closing brace, so the trailing `>f; echo c` is what ends the statement.
/// Every offset in the nested-group skip is observable here.
#[test]
fn nested_group_span_resumes_past_its_closing_brace() {
    // The second group closes at 25; `>f` runs to the `;` at 28.
    span(b"{ echo a; } && { echo b; }>f; echo c", 28, 29);
    // With no separator after the second group, the span runs to end of input.
    span(b"{ echo a; } && { echo b; } && echo c", 36, 36);
    // The tail is a comment: the span ends at the `#` and resumes past the comment.
    span(b"{ echo a; } && { echo b; } # note", 27, 33);
}

/// A group opener after `&&` is not a segment-level block: the cond layer splits
/// the operator, so the line stays one statement and the group is its second
/// part (a block flushed here would run the group past a failing `&&`).
#[test]
fn group_after_and_stays_a_cond_part_not_a_block() {
    let line = b" echo inner && { echo g; }";
    let segs = scan_segments(line, false);
    assert_eq!(segs.len(), 1, "{line:?}");
    match &segs[0] {
        Segment::Statement { cmd, off, .. } => {
            assert_eq!((*off, cmd.len()), (1, 25), "statement slice of {line:?}");
            assert_eq!(
                &line[*off..*off + cmd.len()],
                b"echo inner && { echo g; }".as_slice()
            );
        }
        Segment::Block { .. } => panic!("{line:?} must stay one statement"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
}

/// A brace glued to a word (`a{`) is word data in the group's statement tail,
/// so the span ends at the `;` — not at the brace's closing `}`.
#[test]
fn word_active_brace_in_the_tail_is_not_a_group() {
    span(b"{ echo a; } && a{ b; }", 19, 20);
}

/// A comment glued to a closing brace ends the statement at the `#`: the scan
/// starts past the brace with no word active, so `}`-then-`#` is a comment,
/// not comment data glued to a word.
#[test]
fn glued_comment_after_the_closing_brace_ends_the_statement() {
    span(b"{ echo a; }# note", 11, 17);
    span(b"{ echo a; } && { echo b; }#x", 26, 28);
}

/// A `'…'` span masks its bytes (task #141): a `{`/`}` written inside a span is
/// a word, never a group delimiter, and a span inside a group body is one word
/// whose separator bytes are data.
#[test]
fn quoted_braces_are_not_group_delimiters() {
    for line in [b"'{' x }".as_slice(), b"\"{\" x }".as_slice()] {
        assert!(
            !opens(line, line, 3),
            "a brace written in a span is a word byte, not an opener"
        );
        assert!(
            !opens_at(line, 0),
            "the brace byte in a span opens no group mid-line"
        );
        assert!(
            matches!(
                scan_segments(line, false).first(),
                Some(Segment::Statement { .. })
            ),
            "{line:?} is a plain statement"
        );
    }
    // A group whose body words are spans: the span's `;` is data.
    span(b"{ 'a; b'; }", 11, 11);
    span(b"{ echo 'a' 'b'; }", 17, 17);
}

/// `skip_group`'s pair (resume, closed) for the `{` at `i` of `line`, with a
/// scan state that has a word in progress (the mid-run shape the opener arm
/// hands it).
fn skip_pair(line: &[u8], i: usize) -> (usize, bool) {
    let mut state = ScanState::new();
    state.word_active = true;
    skip_group(line, i, &mut state)
}

/// The group arm answers the pair the bookkeeping needs: a closed group
/// resumes past its closing brace with `true`, an unclosed one resumes at the
/// end of input with `false` — the flag that makes the REPL buffer a
/// `true && { …` line (task #294). A quote inside the body is opaque, so a `}`
/// written in it closes nothing.
#[test]
fn skip_group_reports_the_resume_and_the_closedness() {
    let closed = b"x && { echo g; }";
    assert_eq!(skip_pair(closed, 5), (closed.len(), true));
    let glued = b"x && { echo g;}";
    assert_eq!(skip_pair(glued, 5), (glued.len(), true));
    for unclosed in [
        b"x && { echo g".as_slice(),
        b"x && { { echo n; }",
        b"x && { echo \"a; }\"",
    ] {
        assert_eq!(
            skip_pair(unclosed, 5),
            (unclosed.len(), false),
            "{unclosed:?}"
        );
    }
}

/// The **opener** arm records the flag for the group shape: the mid-run group
/// of a statement is stamped when it never closes, and its closed twin is not
/// (the run-start group is a `Segment::Block` and never reaches this arm).
#[test]
fn the_opener_arm_stamps_an_unclosed_midrun_group() {
    for (line, want) in [
        (b" echo inner && { echo g".as_slice(), true),
        (b" echo inner && { echo g; }", false),
        (b" echo inner && { { echo n; }", true),
    ] {
        let segments = scan_segments(line, false);
        assert_eq!(segments.len(), 1, "{line:?}");
        match &segments[0] {
            Segment::Statement { midrun_open, .. } => {
                assert_eq!(*midrun_open, want, "{line:?}")
            }
            Segment::Block { .. } => panic!("{line:?} must stay one statement"),
            Segment::AsyncInvalid => panic!("unexpected async marker"),
        }
    }
}
