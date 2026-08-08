#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use super::*;

#[test]
fn format_line_single_char_no_newline() {
    let input = b"hello";
    let (line, col, len) = format_line_and_caret(input, 0);
    assert_eq!(line, "hello");
    assert_eq!(col, 0);
    assert_eq!(len, 1);

    let (line2, col2, len2) = format_line_and_caret(input, 3);
    assert_eq!(line2, "hello");
    assert_eq!(col2, 3);
    assert_eq!(len2, 1);

    let (line3, col3, len3) = format_line_and_caret(input, 4);
    assert_eq!(line3, "hello");
    assert_eq!(col3, 4);
    assert_eq!(len3, 1);
}

#[test]
fn format_line_multiline() {
    let input = b"first\nsecond\nthird";
    let (line, col, _len) = format_line_and_caret(input, 0);
    assert_eq!(line, "first");
    assert_eq!(col, 0);

    let (line2, col2, _) = format_line_and_caret(input, 5);
    assert_eq!(line2, "first");
    assert_eq!(col2, 5);

    let (line3, col3, _) = format_line_and_caret(input, 6);
    assert_eq!(line3, "second");
    assert_eq!(col3, 0);

    let (line4, col4, _) = format_line_and_caret(input, 13);
    assert_eq!(line4, "third");
    assert_eq!(col4, 0);
}

#[test]
fn format_line_pos_after_newline_captures_correct_line_end() {
    let input = b"abc\ndef\nghi";
    let (line, col, _) = format_line_and_caret(input, 4);
    assert_eq!(line, "def");
    assert_eq!(col, 0);

    let (line2, col2, _) = format_line_and_caret(input, 6);
    assert_eq!(line2, "def");
    assert_eq!(col2, 2);
}

#[test]
fn format_line_empty_input() {
    let input: &[u8] = b"";
    let (line, col, len) = format_line_and_caret(input, 0);
    assert_eq!(line, "");
    assert_eq!(col, 0);
    assert_eq!(len, 1);
}

#[test]
fn format_line_pos_at_newline() {
    let input = b"hello\nworld";
    let (line, col, _) = format_line_and_caret(input, 5);
    assert_eq!(line, "hello");
    assert_eq!(col, 5);

    let (line2, col2, _) = format_line_and_caret(input, 6);
    assert_eq!(line2, "world");
    assert_eq!(col2, 0);
}

#[test]
fn compute_caret_len_single_char() {
    let input = b"hello";
    assert_eq!(compute_caret_len(input, 0, 0), 1);
    assert_eq!(compute_caret_len(input, 2, 0), 1);
    assert_eq!(compute_caret_len(input, 4, 0), 1);
}

#[test]
fn compute_caret_len_keyword_if() {
    let input = b"if foo; then";
    assert_eq!(compute_caret_len(input, 0, 0), 2);
}

#[test]
fn compute_caret_len_keyword_fi() {
    let input = b"fi";
    assert_eq!(compute_caret_len(input, 0, 0), 2);
}

#[test]
fn compute_caret_len_keyword_then() {
    let input = b"then";
    assert_eq!(compute_caret_len(input, 0, 0), 4);
}

#[test]
fn compute_caret_len_keyword_elif() {
    let input = b"elif foo";
    assert_eq!(compute_caret_len(input, 0, 0), 4);
}

#[test]
fn compute_caret_len_keyword_for() {
    let input = b"for x in";
    assert_eq!(compute_caret_len(input, 0, 0), 3);
}

#[test]
fn compute_caret_len_keyword_while() {
    let input = b"while true";
    assert_eq!(compute_caret_len(input, 0, 0), 5);
}

#[test]
fn compute_caret_len_keyword_until() {
    let input = b"until false";
    assert_eq!(compute_caret_len(input, 0, 0), 5);
}

#[test]
fn compute_caret_len_keyword_done() {
    let input = b"done";
    assert_eq!(compute_caret_len(input, 0, 0), 4);
}

#[test]
fn compute_caret_len_keyword_not_at_boundary() {
    let input = b"difficult";
    assert_eq!(compute_caret_len(input, 2, 0), 1);
}

#[test]
fn compute_caret_len_after_newline() {
    let input = b"hello\nif foo";
    assert_eq!(compute_caret_len(input, 6, 6), 2);
}

#[test]
fn compute_caret_len_multiline_middle() {
    let input = b"first\nsecond\nwhile";
    assert_eq!(compute_caret_len(input, 13, 13), 5);
}

#[test]
fn format_line_non_utf8() {
    let input = b"\xff\xfe";
    let (line, _, _) = format_line_and_caret(input, 0);
    assert_eq!(line, "?");
}

#[test]
fn format_line_trailing_newline_only() {
    let input = b"\nhello";
    let (line, col, _) = format_line_and_caret(input, 1);
    assert_eq!(line, "hello");
    assert_eq!(col, 0);
}

#[test]
fn compute_caret_len_pos_after_line_start() {
    let input = b"abc if foo";
    assert_eq!(compute_caret_len(input, 4, 0), 2);
    assert_eq!(compute_caret_len(input, 5, 0), 1);
}

#[test]
fn compute_caret_len_zero_line_start() {
    let input = b"if";
    assert_eq!(compute_caret_len(input, 0, 0), 2);
}

#[test]
fn compute_caret_len_partial_keyword_match() {
    let input = b"iffy";
    assert_eq!(compute_caret_len(input, 0, 0), 1);
}

#[test]
fn format_line_pos_zero_at_start() {
    let input = b"test line";
    let (line, col, _) = format_line_and_caret(input, 0);
    assert_eq!(line, "test line");
    assert_eq!(col, 0);
}

#[test]
fn format_line_pos_at_end() {
    let input = b"test\nend";
    let (line, col, _) = format_line_and_caret(input, 8);
    assert_eq!(line, "end");
    assert_eq!(col, 3);
}

#[test]
fn compute_caret_len_else_keyword() {
    let input = b"else";
    assert_eq!(compute_caret_len(input, 0, 0), 4);
}

#[test]
fn format_line_multiple_newlines() {
    let input = b"a\nb\nc\nd";
    let (_, col, _) = format_line_and_caret(input, 4);
    assert_eq!(col, 0);

    let (line, _, _) = format_line_and_caret(input, 6);
    assert_eq!(line, "d");
}

#[cfg(debug_assertions)]
#[test]
fn install_debug_hooks_shows_line_and_caret_in_error() {
    use crate::parse::token;

    install_debug_hooks();
    let result = token::tokenize(b"\"unclosed");
    let err = result.unwrap_err();
    let msg = format!("{err:?}");
    assert_eq!(
        msg,
        "\x1b[1munmatched quote\x1b[22m\n\
         ├╴at safe/fdshell/src/error/parse.rs:91:5\n\
         ├╴\"unclosed\n\
         ╰╴^"
    );
}

#[cfg(debug_assertions)]
#[test]
fn install_debug_hooks_shows_correct_caret_position() {
    use crate::parse::token;

    install_debug_hooks();
    let result = token::tokenize(b"abc \"def");
    let err = result.unwrap_err();
    let msg = format!("{err:?}");
    assert_eq!(
        msg,
        "\x1b[1munmatched quote\x1b[22m\n\
         ├╴at safe/fdshell/src/error/parse.rs:91:5\n\
         ├╴abc \"def\n\
         ╰╴    ^"
    );
}

#[cfg(debug_assertions)]
#[test]
fn install_debug_hooks_multiline_error_context() {
    use crate::parse::token;

    install_debug_hooks();
    let result = token::tokenize(b"line1\n\"broken");
    let err = result.unwrap_err();
    let msg = format!("{err:?}");
    assert_eq!(
        msg,
        "\x1b[1munmatched quote\x1b[22m\n\
         ├╴at safe/fdshell/src/error/parse.rs:91:5\n\
         ├╴\"broken\n\
         ╰╴^"
    );
}
