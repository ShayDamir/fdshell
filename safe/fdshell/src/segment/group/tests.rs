#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::scan::{Quotes, ScanState};
use crate::segment::Segment;
use crate::segment::scan_segments;
use alloc::vec::Vec;

/// The group span claims the whole statement it opens: the `{` to the next
/// separator, comment, or end of input, so trailing redirections and the
/// `&&`/`||`/`|` operators belong to the group. `start` is 0 (a group at the
/// start of the line), `pre_resume` and `line_start` are 0. The floor follows
/// the keyword branch's rule: with trailing content on the physical line it is
/// the end of the line's last body region (the line end when the line has no
/// `<<` operator), and `0` when the group ends the line (`Book::rewind`'s
/// deeper-of-two keeps the floor a block before it left).
fn span(line: &[u8], end: usize, resume: usize, floor: usize) {
    let (segment, got_resume, got_floor) = group_block(line, 0, 0, Quotes::new(), 0, 0);
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
    assert_eq!(got_floor, floor, "floor of {line:?}");
}

#[test]
fn group_span_covers_the_statement_after_the_closing_brace() {
    span(b"{ echo a; }", 11, 11, 0);
    span(b"{ echo a; } > out; echo b", 17, 18, 25);
    span(b"{ echo a; } && echo b", 21, 21, 0);
    span(b"{ echo a; } ; echo b", 12, 13, 20);
    span(b"{ echo a; }>out3", 16, 16, 0);
    span(b"{ echo a; } # note", 12, 18, 0);
    span(b"{ { echo n; }; }", 16, 16, 0);
    // A nested group that never closes is opaque: the span runs to end of input.
    span(b"{ echo a; } && { echo b", 23, 23, 0);
}

/// A group that never closes keeps the REPL reading continuation lines: the
/// span runs to the end of the input and reports `closed = false`.
#[test]
fn unclosed_group_is_open() {
    let (segment, resume, _) = group_block(b"{ echo a", 0, 0, Quotes::new(), 0, 0);
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
/// regions: the group's resume starts at its own statement end, and the group
/// ends its physical line, so the floor is `0` and `Book::rewind`'s
/// deeper-of-two keeps the floor the line already carries (task #305). `start`
/// is the group's `{` byte (19), `pre_resume` the here-doc statement's end (17),
/// `line_start` the start of the group's own physical line.
#[test]
fn group_resume_never_reopens_the_group_body() {
    let line = b"cat <<EOF\nhi\nEOF\n; { echo a; } > out";
    let (segment, resume, floor) = group_block(line, 19, 19, Quotes::new(), 17, 19);
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
    assert_eq!(floor, 0);
}

/// The trailing-content arm of the resume rule with a **non-zero** `pre_resume`
/// (task #332, the keyword branch's rule): a here-doc statement before the group
/// owns the line's first region, whose end sits *past* the physical line. The
/// group's resume is its own statement end — the text after its `;` is commands,
/// never a region — and the pre-block region lands in the **floor** only.
/// Flooring the resume with `pre_resume` (master) jumped over the trailing
/// statement and re-opened the region.
///
/// Offsets of `cat <<X; { :; }; echo t\nbodyX\nX`: the `{` sits at 9, the group's
/// statement end (the scan resume) is 16 and `echo t` starts at 17, the physical
/// line ends at 24 and its single region `24..31` ends at 31.
#[test]
fn a_group_with_trailing_content_keeps_its_own_resume_behind_a_pre_block_region() {
    let line = b"cat <<X; { :; }; echo t\nbodyX\nX";
    let (_segment, resume, floor) = group_block(line, 9, 9, Quotes::new(), 31, 0);
    assert_eq!(resume, 16, "the resume is the group's own statement end");
    assert_eq!(floor, 31, "the pre-block region lands in the floor");
}

/// The group's own operators claim regions of its physical line's body stream
/// (task #315, the definer branch's rule): in **append** mode the region bytes
/// travel as the segment's `bodies`, the span end stays at the group's
/// separator, and the resume/floor follow the keyword branch's rule — the
/// trailing branch floors at the end of the line's last region, the line-end
/// branch at `0`.
fn claim(line: &[u8], end: usize, resume: usize, floor: usize, bodies: &[&[u8]]) {
    let (segment, got_resume, got_floor) = group_block(line, 0, 0, Quotes::new(), 0, 0);
    match segment {
        Segment::Block {
            end_pos,
            closed,
            bodies: got,
            ..
        } => {
            assert!(closed, "{line:?} must be a closed group");
            assert_eq!(end_pos, end, "span end of {line:?}");
            let want: Vec<Vec<u8>> = bodies.iter().map(|b| b.to_vec()).collect();
            assert_eq!(got, want, "bodies of {line:?}");
        }
        Segment::Statement { .. } => panic!("{line:?} must be a block"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
    assert_eq!(got_resume, resume, "resume of {line:?}");
    assert_eq!(got_floor, floor, "floor of {line:?}");
}

#[test]
fn a_heredoc_in_the_group_body_claims_its_region() {
    // The group ends its physical line: the resume is pushed past the region.
    claim(b"{ cat <<T; }\nbody\nT", 12, 19, 0, &[b"body\nT"]);
    // The io-number form claims the same region (the descriptor is the parse
    // layer's fact, not the scan's).
    claim(b"{ cat 3<<T; }\nbody\nT", 13, 20, 0, &[b"body\nT"]);
    // Two operators of the group's body, in operator order.
    claim(
        b"{ cat <<T; cat <<U; }\nbT\nT\nbU\nU",
        21,
        31,
        0,
        &[b"bT\nT\n", b"bU\nU"],
    );
    // Trailing content on the group's line: the group keeps its own resume (that
    // text is commands) and the line's final flush floors past the region.
    claim(b"{ cat <<T; }; echo t\nbody\nT", 12, 13, 27, &[b"body\nT"]);
    // The trailing operator takes its **own** region: the group's claim does not
    // move the line's global stream, only the group's part of it.
    claim(
        b"{ cat <<T; }; cat <<U\nbT\nT\nbU\nU",
        12,
        13,
        31,
        &[b"bT\nT\n"],
    );
}

/// The arm of [`claim::claimed_bodies`] one shape cannot reach is recorded, not
/// padded (task #311's rule): a **multi-line** group whose opener line carries
/// no `<<` operator claims nothing at all (the stream is that line's, and the
/// operator is on a later line of the body), so the span is unchanged and no
/// bodies travel — the group's text keeps covering its own lines.
#[test]
fn a_multi_line_group_claims_no_bodies() {
    claim(b"{ echo a\ncat <<T;\n}\nbody\nT", 19, 20, 0, &[]);
    // A multi-line group whose delimiter line sits inside the span keeps the
    // verbatim span too (`tests/group.rs::here_doc_inside_the_group_body`).
    claim(b"{ cat <<EOF\nhi\nEOF\n; }", 22, 22, 0, &[]);
}

/// The byte just after a group's closing brace is inside the span: a body that
/// ends with its required `;` (`{ echo a;}`) keeps that `;` in the span, and the
/// statement-end scan starts *past* the brace, never at the body's `;`.
#[test]
fn span_ends_past_the_closing_brace() {
    span(b"{ echo a;}", 10, 10, 0);
    span(b"{ echo a; }", 11, 11, 0);
    span(b"{ echo a; } ; echo b", 12, 13, 20);
}

/// A second group in the same statement is opaque: the span resumes just past
/// its closing brace, so the trailing `>f; echo c` is what ends the statement.
/// Every offset in the nested-group skip is observable here.
#[test]
fn nested_group_span_resumes_past_its_closing_brace() {
    // The second group closes at 25; `>f` runs to the `;` at 28.
    span(b"{ echo a; } && { echo b; }>f; echo c", 28, 29, 36);
    // With no separator after the second group, the span runs to end of input.
    span(b"{ echo a; } && { echo b; } && echo c", 36, 36, 0);
    // The tail is a comment: the span ends at the `#` and resumes past the comment.
    span(b"{ echo a; } && { echo b; } # note", 27, 33, 0);
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
    span(b"{ echo a; } && a{ b; }", 19, 20, 22);
}

/// A comment glued to a closing brace ends the statement at the `#`: the scan
/// starts past the brace with no word active, so `}`-then-`#` is a comment,
/// not comment data glued to a word.
#[test]
fn glued_comment_after_the_closing_brace_ends_the_statement() {
    span(b"{ echo a; }# note", 11, 17, 0);
    span(b"{ echo a; } && { echo b; }#x", 26, 28, 0);
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
    span(b"{ 'a; b'; }", 11, 11, 0);
    span(b"{ echo 'a' 'b'; }", 17, 17, 0);
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
