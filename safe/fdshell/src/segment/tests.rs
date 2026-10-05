#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use alloc::vec;

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
        Segment::Statement { .. } => panic!("expected Block"),
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
        Segment::Statement { .. } => panic!("expected Block"),
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
        Segment::Statement { .. } => panic!("expected Block"),
    }
}

#[test]
fn scan_statement_no_block() {
    let segments = scan_segments(b"echo hello", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"echo hello"),
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
        Segment::Statement { .. } => panic!("expected Block"),
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
        Segment::Statement { .. } => panic!("expected Block"),
    }
}

#[test]
fn scan_in_block_false_keywords() {
    let segments = scan_segments(b"if x; then y; fi", true);
    assert_eq!(segments.len(), 3);
    match (&segments[0], &segments[1], &segments[2]) {
        (
            Segment::Statement { cmd: s1, .. },
            Segment::Statement { cmd: s2, .. },
            Segment::Statement { cmd: s3, .. },
        ) => {
            assert_eq!(s1, b"if x");
            assert_eq!(s2, b"then y");
            assert_eq!(s3, b"fi");
        }
        _ => panic!("expected Statement segments when in_block=true"),
    }
}

#[test]
fn scan_statement_carries_first_byte_offset() {
    let segments = scan_segments(b"  echo a; echo b", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (
            Segment::Statement {
                cmd: s1, off: o1, ..
            },
            Segment::Statement {
                cmd: s2, off: o2, ..
            },
        ) => {
            assert_eq!(*s1, b"echo a");
            assert_eq!(*o1, 2);
            assert_eq!(*s2, b"echo b");
            assert_eq!(*o2, 10);
        }
        _ => panic!("expected two statements"),
    }
}

#[test]
fn scan_semicolon_separated_statements() {
    let segments = scan_segments(b"echo a; echo b", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (Segment::Statement { cmd: s1, .. }, Segment::Statement { cmd: s2, .. }) => {
            assert_eq!(s1, b"echo a");
            assert_eq!(s2, b"echo b");
        }
        _ => panic!("expected Statement segments"),
    }
}

#[test]
fn scan_comment_skipped() {
    let segments = scan_segments(b"echo hello # comment", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(*s, b"echo hello"),
        _ => panic!("expected Statement"),
    }
}

#[test]
fn scan_hash_mid_word_kept() {
    // `#` inside a word is data, not a comment: the segment keeps it.
    let segments = scan_segments(b"echo a#b", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(*s, b"echo a#b"),
        _ => panic!("expected Statement"),
    }
}

#[test]
fn scan_hash_after_semicolon_is_comment() {
    // A `;` ends the word, so the following `#` begins a comment; only the
    // first statement survives, proving `word_active` is reset at separators.
    let segments = scan_segments(b"echo a;#x", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(*s, b"echo a"),
        _ => panic!("expected Statement"),
    }
}

#[test]
fn scan_comment_on_block_line() {
    // Comment after closing keyword should not prevent detection; the block
    // spans to the end of the line (the comment is part of the span; the
    // tokenizer skips it).
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
            assert!(*closed);
        }
        Segment::Statement { .. } => panic!("expected Block"),
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
        (Segment::Statement { cmd: s1, .. }, Segment::Statement { cmd: s2, .. }) => {
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
        Segment::Statement { .. } => panic!("expected Block"),
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

#[test]
fn scan_block_detects_fi_after_comment_outside_quotes() {
    // Mutant MISSED 4: delete ! in scan_block line 31 affects quote toggling
    let mut in_quote = false;
    let mut start = 2usize; // start after "if"
    let depth = 1u32;
    let (_end, closed) = super::super::comment::scan_block(
        b"if \"hello\" # comment\nfi",
        2,
        &mut in_quote,
        &mut start,
        depth,
    );
    assert!(closed); // fi IS detected after comment, block should be closed
    assert!(!in_quote); // should end with quote state = false
}

#[test]
fn scan_statement_semicolon_inside_dollar_paren() {
    let segments = scan_segments(b"echo $(a; b)", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"echo $(a; b)"),
        Segment::Block { .. } => panic!("expected Statement"),
    }
}

#[test]
fn scan_statement_semicolon_inside_nested_dollar_paren() {
    let segments = scan_segments(b"echo $(a $(b; c) d)", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"echo $(a $(b; c) d)"),
        Segment::Block { .. } => panic!("expected Statement"),
    }
}

#[test]
fn scan_statement_semicolon_inside_backtick() {
    let segments = scan_segments(b"echo `a; b`", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"echo `a; b`"),
        Segment::Block { .. } => panic!("expected Statement"),
    }
}

#[test]
fn scan_statement_semicolon_inside_dollar_paren_with_newline() {
    let segments = scan_segments(b"echo $(a\nb)", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"echo $(a\nb)"),
        Segment::Block { .. } => panic!("expected Statement"),
    }
}

#[test]
fn scan_block_semicolon_inside_dollar_paren() {
    let mut in_quote = false;
    let mut start = 3usize;
    let depth = 1u32;
    let (end, closed) = super::super::comment::scan_block(
        b"if $(a; b); then x; fi",
        3,
        &mut in_quote,
        &mut start,
        depth,
    );
    assert!(closed);
    assert_eq!(end, 23);
}

#[test]
fn scan_statement_plain_paren_inside_dollar_paren() {
    let segments = scan_segments(b"echo $(a (b; c) d)", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"echo $(a (b; c) d)"),
        Segment::Block { .. } => panic!("expected Statement"),
    }
}

#[test]
fn scan_block_plain_paren_inside_dollar_paren() {
    let mut in_quote = false;
    let mut start = 3usize;
    let depth = 1u32;
    let (end, closed) = super::super::comment::scan_block(
        b"if $(a (b; c) d); then x; fi",
        3,
        &mut in_quote,
        &mut start,
        depth,
    );
    assert!(closed);
    assert_eq!(end, 29);
}

#[test]
fn scan_statement_backtick_paren_isolated() {
    let segments = scan_segments(b"echo $(a `b) ; c` d)", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"echo $(a `b) ; c` d)"),
        Segment::Block { .. } => panic!("expected Statement"),
    }
}

// A `(` inside double quotes (within `$( )`) does not open a plain-paren
// scope, so the trailing `;` still splits into a second statement.
#[test]
fn scan_statement_paren_inside_quote_within_dollar_paren() {
    let segments = scan_segments(b"echo $(x \"a(b\"); echo c", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (Segment::Statement { cmd: s1, .. }, Segment::Statement { cmd: s2, .. }) => {
            assert_eq!(s1, b"echo $(x \"a(b\")");
            assert_eq!(s2, b"echo c");
        }
        _ => panic!("expected two Statement segments"),
    }
}

// A `(` inside backticks (within `$( )`) does not open a plain-paren scope.
#[test]
fn scan_statement_paren_inside_backtick_within_dollar_paren() {
    let segments = scan_segments(b"echo $(x `a(b`); echo c", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (Segment::Statement { cmd: s1, .. }, Segment::Statement { cmd: s2, .. }) => {
            assert_eq!(s1, b"echo $(x `a(b`)");
            assert_eq!(s2, b"echo c");
        }
        _ => panic!("expected two Statement segments"),
    }
}

// A top-level plain `(` is not a dollar-paren scope, so the `;` inside it is
// a top-level separator and splits the line.
#[test]
fn scan_statement_top_level_paren_splits_on_semicolon() {
    let segments = scan_segments(b"echo (a; b); echo c", false);
    assert_eq!(segments.len(), 3);
    match (&segments[0], &segments[1], &segments[2]) {
        (
            Segment::Statement { cmd: s1, .. },
            Segment::Statement { cmd: s2, .. },
            Segment::Statement { cmd: s3, .. },
        ) => {
            assert_eq!(s1, b"echo (a");
            assert_eq!(s2, b"b)");
            assert_eq!(s3, b"echo c");
        }
        _ => panic!("expected three Statement segments"),
    }
}

// A plain `(` inside `$( )` adds a dollar-paren depth level, so the `;` after
// the inner `)` is not a top-level separator.
#[test]
fn scan_statement_nested_paren_inside_dollar_paren_protects_semicolon() {
    let segments = scan_segments(b"echo $(a (b); c); echo d", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (Segment::Statement { cmd: s1, .. }, Segment::Statement { cmd: s2, .. }) => {
            assert_eq!(s1, b"echo $(a (b); c)");
            assert_eq!(s2, b"echo d");
        }
        _ => panic!("expected two Statement segments"),
    }
}
// A heredoc statement carries its command line (body-less) plus the body
// region (body lines plus the delimiter line, including its newline).
#[test]
fn scan_statement_heredoc_spans_body() {
    let segments = scan_segments(b"cat <<EOF\nbody\nEOF\necho after", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (
            Segment::Statement {
                cmd: s1,
                off: off1,
                bodies: b1,
            },
            Segment::Statement {
                cmd: s2, off: off2, ..
            },
        ) => {
            assert_eq!(s1, b"cat <<EOF");
            assert_eq!(*off1, 0);
            assert_eq!(b1, &vec![(10, 19)]);
            assert_eq!(s2, b"echo after");
            assert_eq!(*off2, 19);
        }
        _ => panic!("expected two Statement segments"),
    }
}
// The statement after a heredoc starts just past the body's terminating
// newline; the heredoc statement is body-less with its body region attached.
#[test]
fn scan_statement_after_heredoc() {
    let segments = scan_segments(b"a\ncat <<Q\nx\nQ\nb", false);
    assert_eq!(segments.len(), 3);
    match (&segments[1], &segments[2]) {
        (
            Segment::Statement {
                cmd: s1,
                off: off1,
                bodies: b1,
            },
            Segment::Statement {
                cmd: s2, off: off2, ..
            },
        ) => {
            assert_eq!(s1, b"cat <<Q");
            assert_eq!(*off1, 2);
            assert_eq!(b1, &vec![(10, 14)]);
            assert_eq!(s2, b"b");
            assert_eq!(*off2, 14);
        }
        _ => panic!("expected Statement segments"),
    }
}

// A bare `<<` with no delimiter word does not extend the statement.
#[test]
fn scan_statement_bare_heredoc_at_eol_not_extended() {
    let segments = scan_segments(b"cat <<\necho after", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (Segment::Statement { cmd: s1, .. }, Segment::Statement { cmd: s2, .. }) => {
            assert_eq!(s1, b"cat <<");
            assert_eq!(s2, b"echo after");
        }
        _ => panic!("expected two Statement segments"),
    }
}

// `;` after the operator: the run is one statement; with no newline there is
// no body region (the body would follow the whole line).
#[test]
fn scan_statement_heredoc_before_semicolon_not_extended() {
    let segments = scan_segments(b"cat <<EOF ; echo x", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (Segment::Statement { cmd: s1, .. }, Segment::Statement { cmd: s2, .. }) => {
            assert_eq!(s1, b"cat <<EOF");
            assert_eq!(s2, b"echo x");
        }
        _ => panic!("expected two Statement segments"),
    }
}

// A keyword-shaped body line does not close a surrounding if block; only the
// real `fi` does.
#[test]
fn scan_block_heredoc_body_fi_does_not_close() {
    let segments = scan_segments(b"if true; then cat <<EOF\nfi\nEOF\nfi", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 33);
            assert!(*closed);
        }
        _ => panic!("expected Block segment"),
    }
}

// A same-line pid after `wait` is the POSIX builtin: a statement segment,
// not a block.
#[test]
fn scan_wait_pid_is_statement() {
    let segments = scan_segments(b"wait 123", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"wait 123"),
        Segment::Block { .. } => panic!("expected Statement"),
    }
}

// A bare `wait` at end of input is the POSIX builtin.
#[test]
fn scan_bare_wait_is_statement() {
    let segments = scan_segments(b"wait", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"wait"),
        Segment::Block { .. } => panic!("expected Statement"),
    }
}

// A `wait` whose next word is on a subsequent line opens a block.
#[test]
fn scan_wait_block_opens_on_subsequent_line() {
    let segments = scan_segments(b"wait\n readable %rd) x ;;\ndone", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 29);
            assert!(*closed);
        }
        Segment::Statement { .. } => panic!("expected Block"),
    }
}

// A same-line pattern keyword after `wait` opens a block too.
#[test]
fn scan_wait_block_opens_on_same_line_keyword() {
    let segments = scan_segments(b"wait readable %rd) x ;;\ndone", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 28);
            assert!(*closed);
        }
        Segment::Statement { .. } => panic!("expected Block"),
    }
}

// After a function-definition block the scan resumes just past the closing
// `}`; the trailing statement's offset is measured from there.
#[test]
fn scan_function_block_trailing_statement_offset() {
    let segments = scan_segments(b"f() { :; } echo hi", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (
            Segment::Block {
                end_pos, closed, ..
            },
            Segment::Statement { cmd, off, .. },
        ) => {
            assert_eq!(*end_pos, 10);
            assert!(*closed);
            assert_eq!(cmd, b"echo hi");
            assert_eq!(*off, 11);
        }
        _ => panic!("expected Block + Statement"),
    }
}

// The block's exclusive end is the byte after the `}` and the scan resumes
// one byte further: with no separating space, the first byte of the next
// word sits in the resume gap (`}echo` yields the statement `cho hi` — a
// degenerate input both fdshell and bash reject, pinned to pin the resume).
#[test]
fn scan_function_block_resume_gap_eats_glued_byte() {
    let segments = scan_segments(b"f() { :; }echo hi", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (
            Segment::Block {
                end_pos, closed, ..
            },
            Segment::Statement { cmd, off, .. },
        ) => {
            assert_eq!(*end_pos, 10);
            assert!(*closed);
            assert_eq!(cmd, b"cho hi");
            assert_eq!(*off, 11);
        }
        _ => panic!("expected Block + Statement"),
    }
}
