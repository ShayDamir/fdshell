#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::scan::Quotes;
use alloc::vec;

/// A `time [-p]` prefix keeps a keyword block one segment, spanning the prefix
/// too, so the `time` parse hook sees the whole block (`time while … done`).
#[test]
fn scan_time_prefixed_keyword_block() {
    for line in [
        b"time while false; do echo x; done".as_slice(),
        b"time -p if true; then echo y; fi".as_slice(),
        b"time  -p for i in 1 2; do echo $i; done".as_slice(),
    ] {
        let segments = scan_segments(line, false);
        assert_eq!(segments.len(), 1, "{:?}", line);
        match &segments[0] {
            Segment::Block {
                block_start,
                end_pos,
                closed,
                bodies: _,
            } => {
                assert_eq!(*block_start, 0, "{:?}", line);
                assert_eq!(*end_pos, line.len(), "{:?}", line);
                assert!(*closed, "{:?}", line);
            }
            Segment::Statement { cmd, .. } => panic!("expected Block, got {:?}", cmd),
            Segment::AsyncInvalid => panic!("unexpected async marker"),
        }
    }
}

/// A `time` prefix written alone before a `{` joins the group's span; a `time`
/// word with arguments after it stays command data.
#[test]
fn scan_time_prefixed_group_span_starts_at_the_prefix() {
    let segments = scan_segments(b"time { echo a; } > out", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
            bodies: _,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 22);
            assert!(*closed);
        }
        Segment::Statement { cmd, .. } => panic!("expected Block, got {:?}", cmd),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
    // `time echo { …` is a command whose argument is a brace: not a group.
    let segments = scan_segments(b"time echo { a; }", false);
    assert!(segments.len() > 1);
}

#[test]
fn scan_if_block_end_pos() {
    let segments = scan_segments(b"if x; then y; fi", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
            bodies: _,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 16);
            assert!(*closed);
        }
        Segment::Statement { .. } => panic!("expected Block"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
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
            bodies: _,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 30);
            assert!(*closed);
        }
        Segment::Statement { .. } => panic!("expected Block"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
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
            bodies: _,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 26);
            assert!(*closed);
        }
        Segment::Statement { .. } => panic!("expected Block"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
}

#[test]
fn scan_statement_no_block() {
    let segments = scan_segments(b"echo hello", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"echo hello"),
        Segment::Block { .. } => panic!("expected Statement"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
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
            bodies: _,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 18);
            assert!(*closed);
        }
        Segment::Statement { .. } => panic!("expected Block"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
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
            bodies: _,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 32);
            assert!(*closed);
        }
        Segment::Statement { .. } => panic!("expected Block"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
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
            bodies: _,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 26);
            assert!(*closed);
        }
        Segment::Statement { .. } => panic!("expected Block"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
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
            bodies: _,
        } => {
            assert_eq!(*block_start, 0);
            assert!(!*closed);
        }
        Segment::Statement { .. } => panic!("expected Block"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
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
    let mut quotes = Quotes::new();
    let mut start = 2usize; // start after "if"
    let depth = 1u32;
    let (_end, closed) = super::super::comment::scan_block(
        b"if \"hello\" # comment\nfi",
        2,
        &mut quotes,
        &mut start,
        depth,
    );
    assert!(closed); // fi IS detected after comment, block should be closed
    assert!(!quotes.double); // should end with quote state = false
}

#[test]
fn scan_statement_semicolon_inside_dollar_paren() {
    let segments = scan_segments(b"echo $(a; b)", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"echo $(a; b)"),
        Segment::Block { .. } => panic!("expected Statement"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
}

#[test]
fn scan_statement_semicolon_inside_nested_dollar_paren() {
    let segments = scan_segments(b"echo $(a $(b; c) d)", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"echo $(a $(b; c) d)"),
        Segment::Block { .. } => panic!("expected Statement"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
}

#[test]
fn scan_statement_semicolon_inside_backtick() {
    let segments = scan_segments(b"echo `a; b`", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"echo `a; b`"),
        Segment::Block { .. } => panic!("expected Statement"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
}

#[test]
fn scan_statement_semicolon_inside_dollar_paren_with_newline() {
    let segments = scan_segments(b"echo $(a\nb)", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd: s, .. } => assert_eq!(s, b"echo $(a\nb)"),
        Segment::Block { .. } => panic!("expected Statement"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
}

#[test]
fn scan_block_semicolon_inside_dollar_paren() {
    let mut quotes = Quotes::new();
    let mut start = 3usize;
    let depth = 1u32;
    let (end, closed) = super::super::comment::scan_block(
        b"if $(a; b); then x; fi",
        3,
        &mut quotes,
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
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
}

#[test]
fn scan_block_plain_paren_inside_dollar_paren() {
    let mut quotes = Quotes::new();
    let mut start = 3usize;
    let depth = 1u32;
    let (end, closed) = super::super::comment::scan_block(
        b"if $(a (b; c) d); then x; fi",
        3,
        &mut quotes,
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
        Segment::AsyncInvalid => panic!("unexpected async marker"),
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
                ..
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
                ..
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
            bodies: _,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 33);
            assert!(*closed);
        }
        _ => panic!("expected Block segment"),
    }
}

/// The three adjacencies on one physical line allocate the line's body stream
/// in operator order: the pre-block statement takes region 0, the block its own
/// region (append mode: its span stops at `fi`, so the trailing statement's
/// text is not swallowed), and the trailing statement the region left over.
#[test]
fn scan_pre_block_block_and_trailing_share_one_body_stream() {
    let segments = scan_segments(
        b"cat <<X; if cat <<A; then echo y; fi; cat <<B\nbodyX\nX\nbodyA\nA\nbodyB\nB",
        false,
    );
    assert_eq!(segments.len(), 3);
    match (&segments[0], &segments[1], &segments[2]) {
        (
            Segment::Statement {
                cmd: pre,
                off: pre_off,
                bodies: pre_bodies,
                ..
            },
            Segment::Block {
                block_start,
                end_pos,
                closed,
                bodies,
            },
            Segment::Statement {
                cmd: tail,
                off: tail_off,
                bodies: tail_bodies,
                ..
            },
        ) => {
            assert_eq!(pre, b"cat <<X");
            assert_eq!(*pre_off, 0);
            assert_eq!(pre_bodies, &vec![(46, 54)]);
            assert_eq!(*block_start, 8, "the run start, leading space included");
            assert_eq!(*end_pos, 36, "the span stops at the closing keyword");
            assert!(*closed);
            assert_eq!(bodies, &vec![(54, 62)], "the block's own region");
            assert_eq!(tail, b"cat <<B");
            assert_eq!(*tail_off, 38);
            assert_eq!(tail_bodies, &vec![(62, 69)]);
        }
        _ => panic!("expected Statement, Block, Statement"),
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
        Segment::AsyncInvalid => panic!("unexpected async marker"),
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
        Segment::AsyncInvalid => panic!("unexpected async marker"),
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
            bodies: _,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 29);
            assert!(*closed);
        }
        Segment::Statement { .. } => panic!("expected Block"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
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
            bodies: _,
        } => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 28);
            assert!(*closed);
        }
        Segment::Statement { .. } => panic!("expected Block"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
}

/// The `function name { … }` keyword form claims the same span as the POSIX
/// `name() { … }` form: the definer word starts the block, and the trailing
/// statement is scanned from just past the closing `}`.
#[test]
fn scan_function_block_keyword_form_span() {
    let segments = scan_segments(b"function f { echo hi; }; f", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (
            Segment::Block {
                block_start,
                end_pos,
                closed,
                bodies: _,
            },
            Segment::Statement { cmd, off, .. },
        ) => {
            assert_eq!(*block_start, 0);
            assert_eq!(*end_pos, 23);
            assert!(*closed);
            assert_eq!(cmd, b"f");
            assert_eq!(*off, 25);
        }
        _ => panic!("expected Block + Statement"),
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

/// A `;`-terminated heredoc statement before a function definition and a
/// heredoc statement after it: the line's body stream is allocated in operator
/// order, so the pre-block statement takes region 0, the definition claims
/// **none** (its body's heredocs feed the statements inside it — the body is
/// re-scanned), and the trailing statement takes the region left over.
/// (Task #240: on master the trailing statement was fed the pre-block
/// statement's region — `bodyX` plus its delimiter line as content.)
#[test]
fn scan_pre_block_function_and_trailing_share_one_body_stream() {
    check_shapes(
        b"cat <<X; f() { echo hi; }; f; cat <<B\nbodyX\nX\nbodyB\nB",
        &[
            ("S", 0, 0, false, vec![(38, 46)]),
            ("B", 8, 25, true, vec![]),
            ("S", 27, 0, false, vec![]),
            ("S", 30, 0, false, vec![(46, 53)]),
        ],
    );
}

// Two pre-block operators: the trailing statement takes region 2 of the line.
#[test]
fn scan_two_pre_block_heredocs_and_function_take_their_regions() {
    check_shapes(
        b"cat <<X; cat <<Y; f() { echo hi; }; f; cat <<B\nbodyX\nX\nbodyY\nY\nbodyB\nB",
        &[
            ("S", 0, 0, false, vec![(47, 55)]),
            ("S", 9, 0, false, vec![(55, 63)]),
            ("B", 17, 34, true, vec![]),
            ("S", 36, 0, false, vec![]),
            ("S", 39, 0, false, vec![(63, 70)]),
        ],
    );
}

// The `function name { … }` keyword form shares the POSIX form's rule.
#[test]
fn scan_pre_block_function_keyword_form_shares_body_stream() {
    check_shapes(
        b"cat <<X; function f { echo hi; }; f; cat <<B\nbodyX\nX\nbodyB\nB",
        &[
            ("S", 0, 0, false, vec![(45, 53)]),
            ("B", 8, 32, true, vec![]),
            ("S", 34, 0, false, vec![]),
            ("S", 37, 0, false, vec![(53, 60)]),
        ],
    );
}

/// The definition ends the physical line: the resume is pushed past every body
/// region of the line, so the body lines are never emitted as statements (on
/// master `bodyX` ran as a command — rc 1).
#[test]
fn scan_function_block_ending_line_skips_body_regions() {
    check_shapes(
        b"cat <<X; f() { echo hi; }\nbodyX\nX",
        &[
            ("S", 0, 0, false, vec![(26, 33)]),
            ("B", 8, 25, true, vec![]),
        ],
    );
}

// Heredoc statement between the definition and the call: same rule.
#[test]
fn scan_heredoc_between_function_block_and_call_shares_body_stream() {
    check_shapes(
        b"cat <<X; f() { echo hi; }; cat <<B; f\nbodyX\nX\nbodyB\nB",
        &[
            ("S", 0, 0, false, vec![(38, 46)]),
            ("B", 8, 25, true, vec![]),
            ("S", 27, 0, false, vec![(46, 53)]),
            ("S", 36, 0, false, vec![]),
        ],
    );
}

/// The `(kind, off-or-start, end_pos, closed, bodies)` face of every segment a
/// line produces, in scan order (`off` for statements, unused fields zeroed).
type Shape = (&'static str, usize, usize, bool, Vec<(usize, usize)>);

fn check_shapes(line: &[u8], want: &[Shape]) {
    let got: Vec<Shape> = scan_segments(line, false)
        .iter()
        .map(|seg| match seg {
            Segment::Statement { off, bodies, .. } => ("S", *off, 0, false, bodies.clone()),
            Segment::Block {
                block_start,
                end_pos,
                closed,
                bodies,
            } => ("B", *block_start, *end_pos, *closed, bodies.clone()),
            Segment::AsyncInvalid => ("A", 0, 0, false, Vec::new()),
        })
        .collect();
    assert_eq!(got, want, "{line:?}");
}

/// The `{ …; }` group span: the `{` to the end of the statement it opens, so
/// the trailing redirection, `&&`, and `#` comment are outside the block's own
/// text and the next statement starts after them.
#[test]
fn scan_group_statement_span() {
    check_block(b"{ echo a; }", (0, 11, true));
    check_block(b"{ echo a; } && echo b", (0, 21, true));
    check_block(b"{ { echo n; }; }", (0, 16, true));
    check_block(b"{ cat <<EOF\nhi\nEOF\n; }", (0, 22, true));
    check_block(b"{ echo a", (0, 8, false));
    check_segments(b"echo a; { echo b; }", &[(8, 19, true)]);
    check_segments(b"{ echo a; } > out; echo b", &[(0, 17, true)]);
    check_segments(b"{ echo a; } ; echo b", &[(0, 12, true)]);
    check_segments(b"{ echo a; } # note", &[(0, 12, true)]);
    check_segments(b"f() { { echo n; }; }", &[(0, 20, true)]);
}

/// A group at the start of the line, with the span end and closed flag.
fn check_block(line: &[u8], want: (usize, usize, bool)) {
    let segments = scan_segments(line, false);
    assert_eq!(segments.len(), 1, "{line:?}");
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
            bodies: _,
        } => assert_eq!((*block_start, *end_pos, *closed), want, "{line:?}"),
        Segment::Statement { .. } => panic!("{line:?} must be a group block"),
        Segment::AsyncInvalid => panic!("{line:?} must be a group block"),
    }
}

/// The block spans a line produces, in order (statements are not listed).
fn check_segments(line: &[u8], want: &[(usize, usize, bool)]) {
    let blocks: Vec<(usize, usize, bool)> = scan_segments(line, false)
        .iter()
        .filter_map(|s| match s {
            Segment::Block {
                block_start,
                end_pos,
                closed,
                bodies: _,
            } => Some((*block_start, *end_pos, *closed)),
            Segment::Statement { .. } => None,
            Segment::AsyncInvalid => None,
        })
        .collect();
    assert_eq!(blocks, want, "{line:?}");
}

/// A brace-shaped word is a command word, never a group opener: brace
/// expansion, the glued command name, and the glued `{;` (a bash syntax
/// error) stay statements, as does a group inside a pipeline stage.
#[test]
fn scan_brace_words_are_statements() {
    let lines: &[&[u8]] = &[
        b"echo {",
        b"echo }",
        b"{echo hi; }",
        b"{; echo a; }",
        b"{a,b} echo x",
        b"{1..5} echo x",
        b"{}; echo hi",
        b"echo a| { echo b; }",
    ];
    for line in lines {
        let segments = scan_segments(line, false);
        assert!(
            segments
                .iter()
                .all(|s| matches!(s, Segment::Statement { .. })),
            "{line:?} must be statements"
        );
        assert!(!segments.is_empty(), "{line:?}");
    }
}

/// Inside a block body (`in_block = true`) the group braces are plain words:
/// the body's own statements run as written.
#[test]
fn scan_group_in_block_is_statements() {
    let segments = scan_segments(b"{ echo a; }", true);
    // The braces are words: the body's runs flush as the line's statements.
    assert_eq!(segments.len(), 2);
    assert!(matches!(segments[0], Segment::Statement { .. }));
}

// Task #130: a `;` (or a newline) written inside a quoted `$( … )`, `${ … }`
// or backtick region is region content, so the statement is not split there.
#[test]
fn separator_inside_a_quoted_region_does_not_split_a_statement() {
    let cases: &[(&[u8], &[&[u8]])] = &[
        (
            b"echo \"$(printf \"a; b\")\"",
            &[b"echo \"$(printf \"a; b\")\""],
        ),
        (
            b"echo \"`printf \"a; b\"`\"",
            &[b"echo \"`printf \"a; b\"`\""],
        ),
        (b"echo \"${u:-\"a; b\"}\"", &[b"echo \"${u:-\"a; b\"}\""]),
        (
            b"echo \"$(printf \"a; b\")\"; echo after",
            &[b"echo \"$(printf \"a; b\")\"", b"echo after"],
        ),
        (
            b"echo \"$(printf \"a; b\")\" && echo ok",
            &[b"echo \"$(printf \"a; b\")\" && echo ok"],
        ),
        (
            b"echo \"$(printf \"a\nb\")\"",
            &[b"echo \"$(printf \"a\nb\")\""],
        ),
    ];
    for (line, want) in cases {
        let got = statements(line);
        let want: Vec<Vec<u8>> = want.iter().map(|w| w.to_vec()).collect();
        assert_eq!(got, want, "{line:?}");
    }
}

/// Each segment's bytes: a statement's command text, a block's whole span.
fn statements(line: &[u8]) -> Vec<Vec<u8>> {
    scan_segments(line, false)
        .iter()
        .map(|segment| match segment {
            Segment::Statement { cmd, .. } => cmd.to_vec(),
            Segment::Block {
                block_start,
                end_pos,
                ..
            } => line.get(*block_start..*end_pos).unwrap_or(b"").to_vec(),
            Segment::AsyncInvalid => Vec::new(),
        })
        .collect()
}

/// A `'…'` span is one opaque literal word (task #141): the separator, pipe,
/// `&&` or newline written in it never splits a statement, and the span's
/// metacharacters are data.
#[test]
fn separators_inside_a_single_quote_span_are_data() {
    let cases: &[(&[u8], &[&[u8]])] = &[
        (b"echo 'a; b'", &[b"echo 'a; b'"]),
        (b"echo 'a\nb'", &[b"echo 'a\nb'"]),
        (b"echo 'a && b'", &[b"echo 'a && b'"]),
        (b"echo 'a | b'", &[b"echo 'a | b'"]),
        (b"echo 'a; b'; echo after", &[b"echo 'a; b'", b"echo after"]),
        (b"echo 'a' && echo ok", &[b"echo 'a' && echo ok"]),
        // An empty span is a word of its own, and it separates nothing.
        (b"echo '' ; echo after", &[b"echo ''", b"echo after"]),
        // The span and a `"` region nest independently: the `'` in `"` is data
        // and the `"` in `'` is data.
        (b"echo \"a'b;c\"; echo x", &[b"echo \"a'b;c\"", b"echo x"]),
        (b"echo 'a\"b;c'; echo x", &[b"echo 'a\"b;c'", b"echo x"]),
    ];
    for (line, want) in cases {
        let got = statements(line);
        let want: Vec<Vec<u8>> = want.iter().map(|w| w.to_vec()).collect();
        assert_eq!(got, want, "{line:?}");
    }
}

/// A block keyword written in a `'…'` span opens and closes nothing (task
/// #141): the block below is closed by the real `fi`, and the quoted `then`,
/// `done`, `fi` are command words.
#[test]
fn quoted_keywords_do_not_open_or_close_blocks() {
    // `'fi'` as an argument: the statement is one command, the block is intact.
    check_block(b"if true; then echo 'fi'; fi", (0, 27, true));
    // `'then'` is not the opener: the run is a plain command statement.
    assert_eq!(
        statements(b"if true; 'then' echo x; fi"),
        vec![b"if true; 'then' echo x; fi".to_vec()],
        "the quoted `then` opens nothing, so the whole line is one statement"
    );
    // A quoted `done` in a loop body does not close it.
    check_block(b"while true; do echo 'done'; done", (0, 32, true));
}

/// Task #232: a keyword block that opens after `&&` / `||` is a cond part
/// inside the run: the run stays **one statement** covering the whole block
/// text (the block's own `;` are not top-level separators), every opener
/// spelling joins the run, the `time` prefix rides inside it, and the trailing
/// `;` after the closing keyword ends the statement as usual.
#[test]
fn midrun_keyword_block_stays_one_statement_in_the_run() {
    for line in [
        b"true && while :; do echo hi; break; done".as_slice(),
        b"echo a && if true; then echo b; fi".as_slice(),
        b"true && for i in 1 2; do echo $i; done".as_slice(),
        b"true && until :; do echo u; break; done".as_slice(),
        b"true && case x in x) echo cx;; esac".as_slice(),
        b"true && f() { echo F; } && f".as_slice(),
        b"true && time while :; do :; break; done".as_slice(),
        b"x && time -p while :; do :; break; done".as_slice(),
        b"true && time { echo g; }".as_slice(),
        b"true && while :; do\necho hi\nbreak\ndone".as_slice(),
        b"true && while :; do echo hi; break; done && echo y".as_slice(),
    ] {
        assert_eq!(statements(line), vec![line.to_vec()], "{line:?}");
    }
    // The closer's `;` ends the statement: the tail is its own statement.
    assert_eq!(
        statements(b"true && while :; do echo hi; break; done; echo tail"),
        vec![
            b"true && while :; do echo hi; break; done".to_vec(),
            b"echo tail".to_vec(),
        ]
    );
    // A keyword-shaped *argument* mid-run stays word data: the run splits at
    // its `;` as before (the `|` stage is task #292's gap, unchanged).
    assert_eq!(
        statements(b"echo a | while :; do echo b; break; done"),
        vec![
            b"echo a | while :".to_vec(),
            b"do echo b".to_vec(),
            b"break".to_vec(),
            b"done".to_vec(),
        ]
    );
    // A keyword at a line start in an argument position is word data too.
    assert_eq!(
        statements(b"echo if\ntrue"),
        vec![b"echo if".to_vec(), b"true".to_vec()]
    );
}

/// Task #232 regression: the run-start block path is untouched by the mid-run
/// span — a run-start keyword is still a `Segment::Block` with its span, and a
/// block that stays unclosed mid-run is a plain statement (the parse layer
/// reports the missing closer).
#[test]
fn run_start_blocks_and_unclosed_midruns_keep_their_shapes() {
    check_block(b"while :; do echo hi; break; done", (0, 32, true));
    check_block(b"if true; then echo t; fi", (0, 24, true));
    // Unclosed mid-run: one statement, the parse reports the missing closer.
    assert_eq!(
        statements(b"true && while :; do echo hi"),
        vec![b"true && while :; do echo hi".to_vec()]
    );
    assert_eq!(statements(b"true && if"), vec![b"true && if".to_vec()]);
}

/// The plain `&` written **inside a block's text** is a separator inside the
/// span, not the statement split's async marker (task #354): the block-opener
/// gate owns the run, so every closer form — `& ;;`, `&;;`, `& ;;esac`,
/// `& esac`, the glued multi-line arm, the `& }` of a group, the `&` of a
/// function body, the `&`-terminated condition — yields one closed
/// `Segment::Block`, and the `&` stays in the span's text.
#[test]
fn scan_ampersand_inside_block_text_stays_in_the_span() {
    for line in [
        b"case x in x) echo a & ;; esac".as_slice(),
        b"case x in x) echo a &;; esac".as_slice(),
        b"case x in x) echo a & ;;esac".as_slice(),
        b"case x in x) echo a & esac".as_slice(),
        b"case x in x) echo a &\nesac".as_slice(),
        b"if true & then echo x; fi".as_slice(),
        b"if true; then echo x & fi".as_slice(),
        b"for i in 1; do echo hi & done".as_slice(),
        b"while :; do echo hi & break; done".as_slice(),
        b"{ echo hi & }".as_slice(),
        b"f(){ echo hi & }".as_slice(),
    ] {
        let segments = scan_segments(line, false);
        assert_eq!(
            segments.len(),
            1,
            "{:?} -> {} segments",
            line,
            segments.len()
        );
        match &segments[0] {
            Segment::Block {
                block_start,
                end_pos,
                closed,
                ..
            } => {
                assert_eq!(*block_start, 0, "{:?}", line);
                assert_eq!(*end_pos, line.len(), "{:?}", line);
                assert!(*closed, "span not closed: {:?}", line);
            }
            Segment::Statement { cmd, .. } => {
                panic!("expected one closed Block, got a statement split: {cmd:?}")
            }
            Segment::AsyncInvalid => panic!("unexpected async marker: {:?}", line),
        }
    }
    // A function block plus a tail command: the span ends at the `}`, the tail
    // is its own statement.
    let segments = scan_segments(b"f(){ echo hi & }; f", false);
    assert_eq!(segments.len(), 2);
    match &segments[0] {
        Segment::Block {
            block_start,
            end_pos,
            closed,
            ..
        } => {
            assert_eq!((*block_start, *end_pos, *closed), (0, 16, true));
        }
        Segment::Statement { .. } => panic!("expected the function block first"),
        Segment::AsyncInvalid => panic!("expected the function block first"),
    }
    match &segments[1] {
        Segment::Statement { cmd, .. } => assert_eq!(*cmd, b"f".as_slice()),
        Segment::Block { .. } => panic!("expected the tail statement"),
        Segment::AsyncInvalid => panic!("expected the tail statement"),
    }
}

/// The gate does **not** widen the statement split: a plain `&` outside any
/// block is the async marker exactly as before (the span's text loses the `&`),
/// and a `;;` behind it is the invalid split the `peek` guard catches — the
/// guard is untouched, so `echo a & ;; echo b` is still `AsyncInvalid` at
/// statement level and inside a block body (`in_block = true`).
#[test]
fn scan_ampersand_outside_a_block_is_unchanged() {
    let segments = scan_segments(b"echo hi &", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Statement { cmd, .. } => assert_eq!(*cmd, b"echo hi".as_slice()),
        Segment::Block { .. } => panic!("expected the async statement"),
        Segment::AsyncInvalid => panic!("expected the async statement"),
    }
    for in_block in [false, true] {
        for line in [
            b"echo a & ;; echo b".as_slice(),
            b"echo a &;;echo b".as_slice(),
        ] {
            let segments = scan_segments(line, in_block);
            assert_eq!(segments.len(), 2, "{:?} in_block={in_block}", line);
            assert!(
                matches!(segments[0], Segment::AsyncInvalid),
                "the peek guard must still fire: {:?} in_block={in_block}",
                line
            );
        }
    }
    // A case arm whose `;;` is behind the `&` never reaches the guard: the arm's
    // scan owns the run, and with no `esac` the span simply stays open (the
    // script-level parse error, bash's rc 2).
    let segments = scan_segments(b"case x in x) echo a & ;; echo b", false);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Block { closed, .. } => assert!(!*closed, "the span must stay open"),
        Segment::Statement { .. } => panic!("expected the open case span"),
        Segment::AsyncInvalid => panic!("expected the open case span"),
    }
}

/// The plain-`&` split (task #313) is the third adjacency on a physical line:
/// the backgrounded statement and the statement written **behind** the `&`
/// share the line's body stream in operator order, exactly as the pre-block
/// and trailing statements of
/// [`scan_pre_block_block_and_trailing_share_one_body_stream`] do. Before the
/// fix the `&` flush parked the cursor past the whole line's regions, so the
/// suffix's body lines were scanned as statements.
#[test]
fn scan_ampersand_separated_heredocs_share_one_body_stream() {
    let segments = scan_segments(b"cat <<A & cat <<B\nx\nA\ny\nB", false);
    assert_eq!(segments.len(), 2);
    match (&segments[0], &segments[1]) {
        (
            Segment::Statement {
                cmd: head,
                off: head_off,
                bodies: head_bodies,
                background: head_bg,
            },
            Segment::Statement {
                cmd: tail,
                off: tail_off,
                bodies: tail_bodies,
                background: tail_bg,
            },
        ) => {
            assert_eq!(head, b"cat <<A");
            assert_eq!(*head_off, 0);
            assert_eq!(head_bodies, &vec![(18, 22)], "region 0: `x` + `A`");
            assert!(*head_bg, "the `&` backgrounds the run it terminates");
            assert_eq!(tail, b"cat <<B");
            assert_eq!(*tail_off, 10);
            assert_eq!(tail_bodies, &vec![(22, 25)], "region 1: `y` + `B`");
            assert!(!*tail_bg, "the suffix runs in the foreground");
        }
        _ => panic!("expected two Statement segments"),
    }
}
