#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::parse::line::ParsedLine;
use crate::parse::token::tokenize_statement;
use alloc::vec::Vec;
use sys::{Origin, Position, ScriptText};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        sys::ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// Run the hook the way `inner_parse` does: tokenize the statement, then ask
/// `detect`. `None` means the statement is not a `time` prefix statement.
fn detect_line(line: &[u8]) -> Option<TimeBlock> {
    let text = stext(line);
    let raw = tokenize_statement(text.as_bytes().unwrap()).unwrap();
    match super::detect(&raw, &text).unwrap() {
        Some(ParsedLine::Time(block)) => Some(block),
        Some(_) => None,
        None => None,
    }
}

/// The inner statement's text, as bytes.
fn inner(block: &TimeBlock) -> Vec<u8> {
    block.inner_text.as_bytes().unwrap().to_vec()
}

#[test]
fn plain_command_is_timed() {
    let block = detect_line(b"time ls -l").unwrap();
    assert!(!block.posix);
    assert!(matches!(block.inner.as_deref(), Some(ParsedLine::Cmd(_))));
    assert_eq!(inner(&block), b"ls -l");
}

#[test]
fn inner_text_keeps_the_token_positions() {
    let block = detect_line(b"time ls /nope_dir").unwrap();
    // The inner text is a subslice of the statement, so its first byte keeps
    // its column: the `l` of `ls` sits at column 6.
    assert_eq!(block.inner_text.start.line, 1);
    assert_eq!(block.inner_text.start.column, 6);
    assert_eq!(inner(&block), b"ls /nope_dir");
}

#[test]
fn dash_p_selects_the_posix_layout() {
    let block = detect_line(b"time -p ls").unwrap();
    assert!(block.posix);
    assert_eq!(inner(&block), b"ls");
}

#[test]
fn dash_p_only_before_the_statement_counts() {
    // `-p` after the inner statement is one of its arguments, not the flag.
    let block = detect_line(b"time ls -p").unwrap();
    assert!(!block.posix);
    assert_eq!(inner(&block), b"ls -p");
}

#[test]
fn prefix_run_collapses_to_one_report() {
    let block = detect_line(b"time time -p time ls").unwrap();
    assert!(block.posix);
    assert_eq!(inner(&block), b"ls");
    // Only the first `-p` is the flag; a second one is the statement's word.
    let block = detect_line(b"time -p -p ls").unwrap();
    assert!(block.posix);
    assert_eq!(inner(&block), b"-p ls");
    // A `time` prefix behind the flag is collapsed, not a statement of its own.
    let block = detect_line(b"time -p time ls").unwrap();
    assert!(block.posix);
    assert_eq!(inner(&block), b"ls");
}

/// Each `time` takes the `-p` written right behind it, so a run of prefixed
/// prefixes leaves no `-p` behind as a command word: `time -p time -p :` times the
/// `:` (bash: one POSIX-layout report, status 0), not a command named `-p`.
#[test]
fn every_prefix_takes_its_own_flag() {
    for (line, want) in [
        (b"time -p time -p :".as_slice(), b":".as_slice()),
        (b"time time -p time -p ls".as_slice(), b"ls".as_slice()),
        (b"time -p time -p time -p ls".as_slice(), b"ls".as_slice()),
    ] {
        let block = detect_line(line).unwrap();
        assert!(block.posix, "{line:?}");
        assert_eq!(inner(&block), want, "{line:?}");
    }
}

/// The two scorers of the prefix run must strip the same bytes: the segment
/// scanner's [`time_prefix_len`] and this hook's token loop. Any drift leaves a
/// prefix word at the head of the inner statement (a command named `-p`).
#[test]
fn parse_prefix_run_matches_time_prefix_len() {
    for line in [
        b"time ls".as_slice(),
        b"time -p ls".as_slice(),
        b"time time -p ls".as_slice(),
        b"time -p time -p ls".as_slice(),
        b"time time time :".as_slice(),
        b"time -p -p ls".as_slice(),
        b"time -p ls -p".as_slice(),
    ] {
        let block = detect_line(line).unwrap();
        let prefix = crate::keywords::time_prefix_len(line).unwrap_or(0);
        assert_eq!(
            inner(&block),
            line.get(prefix..).unwrap_or(b"").trim_ascii(),
            "{line:?}"
        );
    }
}

#[test]
fn bare_time_has_no_inner_statement() {
    let block = detect_line(b"time").unwrap();
    assert!(block.inner.is_none());
    assert!(block.inner_text.as_bytes().unwrap().is_empty());
    let block = detect_line(b"time -p").unwrap();
    assert!(block.posix);
    assert!(block.inner.is_none());
}

#[test]
fn quoted_time_is_the_utility_not_the_reserved_word() {
    // XCU 2.4: a reserved word has none of its characters quoted.
    assert!(detect_line(b"\"time\" ls").is_none());
    assert!(detect_line(b"'time' ls").is_none());
    assert!(detect_line(b"t\"i\"me ls").is_none());
    assert!(detect_line(b"\"time -p\" ls").is_none());
}

#[test]
fn quoted_dash_p_is_an_argument() {
    let block = detect_line(b"time \"-p\" ls").unwrap();
    assert!(!block.posix);
    assert_eq!(inner(&block), b"\"-p\" ls");
}

#[test]
fn keyword_blocks_are_timed_whole() {
    let tails: [&[u8]; 5] = [
        b"if true; then echo y; fi",
        b"while false; do echo x; done",
        b"for i in 1 2; do echo $i; done",
        b"until false; do echo u; done",
        b"case x in x) echo X;; esac",
    ];
    for tail in tails {
        let mut full = b"time ".to_vec();
        full.extend_from_slice(tail);
        let block = detect_line(&full).unwrap();
        assert_eq!(inner(&block), tail, "in {full:?}");
    }
}

#[test]
fn block_prefix_keeps_the_block_shape() {
    let block = detect_line(b"time -p while false; do echo x; done").unwrap();
    assert!(block.posix);
    assert!(matches!(block.inner.as_deref(), Some(ParsedLine::While(_))));
    assert_eq!(inner(&block), b"while false; do echo x; done");
}

#[test]
fn group_block_is_timed_with_its_tail_redirect() {
    let block = detect_line(b"time { echo a; } > /dev/null").unwrap();
    assert!(matches!(block.inner.as_deref(), Some(ParsedLine::Group(_))));
    assert_eq!(inner(&block), b"{ echo a; } > /dev/null");
}

#[test]
fn pipeline_is_timed_whole() {
    let block = detect_line(b"time sleep 0.2 | cat").unwrap();
    assert!(matches!(
        block.inner.as_deref(),
        Some(ParsedLine::Pipeline(_))
    ));
    assert_eq!(inner(&block), b"sleep 0.2 | cat");
}

#[test]
fn arithmetic_expression_is_timed() {
    let block = detect_line(b"time ((1))").unwrap();
    assert!(matches!(
        block.inner.as_deref(),
        Some(ParsedLine::ArithCommand(_))
    ));
}

#[test]
fn builtin_and_alias_words_are_timed() {
    let block = detect_line(b"time cd /tmp").unwrap();
    assert!(matches!(block.inner.as_deref(), Some(ParsedLine::Cmd(_))));
    assert_eq!(inner(&block), b"cd /tmp");
    let block = detect_line(b"time true").unwrap();
    assert_eq!(inner(&block), b"true");
}

#[test]
fn prefix_after_the_inner_is_not_stripped() {
    // `time` is only a prefix statement at the statement's start.
    assert!(detect_line(b"echo time").is_none());
    assert!(detect_line(b"echo time ls").is_none());
}

/// The inner text's span ends at the last token, not at the end of the text:
/// the bytes a statement's tokens do not cover (trailing whitespace) belong to
/// the block span, which only a block opener takes. `wait` is the word that
/// scores `Some(0)` in the depth rule (the POSIX builtin, not the event-case
/// block opener) and a closer scores a negative delta — both are statements.
#[test]
fn inner_span_ends_at_the_last_token() {
    assert_eq!(inner(&detect_line(b"time ls -l  ").unwrap()), b"ls -l");
    assert_eq!(inner(&detect_line(b"time wait  ").unwrap()), b"wait");
    assert_eq!(inner(&detect_line(b"time -p wait x  ").unwrap()), b"wait x");
    assert_eq!(inner(&detect_line(b"time fi  ").unwrap()), b"fi");
}

/// A quoted block keyword scores a block delta on its quote-folded word, so only
/// the XCU 2.4 "none of the characters is quoted" test keeps its statement on
/// the token span: the block span would reach the trailing whitespace its tokens
/// do not cover.
#[test]
fn quoted_block_keyword_inner_takes_the_token_span() {
    let block = detect_line(b"time \"while\" false; do :; done  ").unwrap();
    assert_eq!(inner(&block), b"\"while\" false; do :; done");
    let block = detect_line(b"time -p \"if\" true; then :; fi  ").unwrap();
    assert!(block.posix);
    assert_eq!(inner(&block), b"\"if\" true; then :; fi");
    // A quoted word that is no keyword at all is a plain command.
    assert_eq!(inner(&detect_line(b"time \"ls\"  ").unwrap()), b"\"ls\"");
}
