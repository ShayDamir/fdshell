#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

#[test]
fn scan_if_block_end_pos() {
    let segments = scan_segments(b"if x; then y; fi", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 16);
            assert!(*closed);
        }
        Segment::Statement(_) => panic!("expected Block"),
    }
}

#[test]
fn scan_for_block_end_pos() {
    let segments = scan_segments(b"for i in a b; do echo $i; done", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 30);
            assert!(*closed);
        }
        Segment::Statement(_) => panic!("expected Block"),
    }
}

#[test]
fn scan_case_block_end_pos() {
    let segments = scan_segments(b"case x in a) echo 1;; esac", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 26);
            assert!(*closed);
        }
        Segment::Statement(_) => panic!("expected Block"),
    }
}

#[test]
fn scan_statement_no_block() {
    let segments = scan_segments(b"echo hello", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement(s) => assert_eq!(s, b"echo hello"),
        Segment::Block { .. } => panic!("expected Statement"),
    }
}

#[test]
fn scan_if_and_for_produce_different_end_pos() {
    let if_segs = scan_segments(b"if x; then y; fi", false);
    let for_segs = scan_segments(b"for i in a b; do echo $i; done", false);
    match (&if_segs[0], &for_segs[0]) {
        (
            Segment::Block {
                end_pos: if_end, ..
            },
            Segment::Block {
                end_pos: for_end, ..
            },
        ) => {
            assert_ne!(
                *if_end, *for_end,
                "if and for must produce different end_pos"
            );
        }
        _ => panic!("expected Block segments"),
    }
}

#[test]
fn scan_case_and_for_produce_different_end_pos() {
    let case_segs = scan_segments(b"case x in a) echo 1;; esac", false);
    let for_segs = scan_segments(b"for i in a b; do echo $i; done", false);
    match (&case_segs[0], &for_segs[0]) {
        (
            Segment::Block {
                end_pos: case_end, ..
            },
            Segment::Block {
                end_pos: for_end, ..
            },
        ) => {
            assert_ne!(
                *case_end, *for_end,
                "case and for must produce different end_pos"
            );
        }
        _ => panic!("expected Block segments"),
    }
}

#[test]
fn scan_if_with_leading_whitespace() {
    let segments = scan_segments(b"  if x; then y; fi", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 18);
            assert!(*closed);
        }
        Segment::Statement(_) => panic!("expected Block"),
    }
}

#[test]
fn scan_for_with_leading_whitespace() {
    let segments = scan_segments(b"  for i in a b; do echo $i; done", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 32);
            assert!(*closed);
        }
        Segment::Statement(_) => panic!("expected Block"),
    }
}

#[test]
fn scan_in_block_false_keywords() {
    let segments = scan_segments(b"if x; then y; fi", true);
    assert_eq!(segments.len(), 3);
    match (&segments[0], &segments[1], &segments[2]) {
        (Segment::Statement(s1), Segment::Statement(s2), Segment::Statement(s3)) => {
            assert_eq!(s1, b"if x");
            assert_eq!(s2, b"then y");
            assert_eq!(s3, b"fi");
        }
        _ => panic!("expected Statement segments when in_block=true"),
    }
}

#[test]
fn scan_semicolon_separated_statements() {
    let segments = scan_segments(b"echo a; echo b", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (Segment::Statement(s1), Segment::Statement(s2)) => {
            assert_eq!(s1, b"echo a");
            assert_eq!(s2, b"echo b");
        }
        _ => panic!("expected Statement segments"),
    }
}

#[test]
fn scan_comment_skipped() {
    let segments = scan_segments(b"echo hello # comment", false);
    assert!(
        segments.is_empty(),
        "comment should skip all content to end of line"
    );
}

#[test]
fn scan_comment_on_block_line() {
    // Comment after closing keyword prevents scan_block from finding it,
    // so the block is not considered closed.
    let segments = scan_segments(b"if x; then y; fi # comment", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 26);
            assert!(!*closed);
        }
        Segment::Statement(_) => panic!("expected Block"),
    }
}

#[test]
fn scan_empty_line() {
    let segments = scan_segments(b"", false);
    assert!(segments.is_empty());
}

#[test]
fn scan_only_whitespace() {
    let segments = scan_segments(b"   ", false);
    assert!(segments.is_empty());
}

#[test]
fn scan_newline_separated() {
    let segments = scan_segments(b"echo a\necho b", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (Segment::Statement(s1), Segment::Statement(s2)) => {
            assert_eq!(s1, b"echo a");
            assert_eq!(s2, b"echo b");
        }
        _ => panic!("expected Statement segments"),
    }
}

#[test]
fn scan_block_not_closed() {
    let segments = scan_segments(b"if x; then y;", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos: _,
            closed,
        } => {
            assert_eq!(*block_start, 0);
            assert!(!*closed);
        }
        Segment::Statement(_) => panic!("expected Block"),
    }
}

#[test]
fn scan_kw_len_arithmetic_with_whitespace() {
    // With leading whitespace, the position offset must be correct.
    // "  if" → block_start=0, leading_ws=2, kw_len=2, after_kw=4
    // "  for" → block_start=0, leading_ws=2, kw_len=3, after_kw=5
    // These different after_kw values must cause different end_pos values.
    let if_segs = scan_segments(b"  if x; then y; fi", false);
    let for_segs = scan_segments(b"  for i in a b; do echo $i; done", false);
    match (&if_segs[0], &for_segs[0]) {
        (
            Segment::Block {
                block_start: if_start,
                end_pos: if_end,
                ..
            },
            Segment::Block {
                block_start: for_start,
                end_pos: for_end,
                ..
            },
        ) => {
            assert_eq!(*if_start, 0, "block_start should be 0 (start of line)");
            assert_eq!(*for_start, 0, "block_start should be 0 (start of line)");
            assert_ne!(
                *if_end, *for_end,
                "different keywords must produce different end_pos"
            );
        }
        _ => panic!("expected Block segments"),
    }
}
