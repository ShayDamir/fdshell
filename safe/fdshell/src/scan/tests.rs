#![allow(clippy::indexing_slicing)]
use super::*;

// `$( )` is consumed together, advancing two positions and raising depth.
#[test]
fn advance_dollar_paren_advances_two() {
    let mut s = ScanState::new();
    let next = s.advance(b"$(x", 0);
    assert_eq!(next, 2, "$ ( must be consumed together");
    assert_eq!(s.dollar_paren_depth, 1);
}

// `$var` (no paren) advances one position and does not raise depth.
#[test]
fn advance_dollar_var_advances_one() {
    let mut s = ScanState::new();
    let next = s.advance(b"$x", 0);
    assert_eq!(next, 1);
    assert_eq!(s.dollar_paren_depth, 0);
}

// A double quote toggles `in_quote` on and off.
#[test]
fn advance_quote_open_close() {
    let line = b"a\"b\"c";
    let mut s = ScanState::new();
    s.advance(line, 0); // 'a'
    assert!(!s.in_quote);
    s.advance(line, 1); // '"'
    assert!(s.in_quote);
    s.advance(line, 2); // 'b'
    assert!(s.in_quote);
    s.advance(line, 3); // '"'
    assert!(!s.in_quote);
}

// A double quote toggles `in_quote` even inside a backtick span (existing behavior).
#[test]
fn advance_quote_inside_backtick_toggles_quote() {
    let line = b"`\"";
    let mut s = ScanState::new();
    s.advance(line, 0); // '`' opens backtick
    assert!(s.in_backtick);
    assert!(!s.in_quote);
    s.advance(line, 1); // '"' inside backtick
    assert!(
        s.in_quote,
        "a double quote toggles in_quote even inside a backtick span"
    );
}

// Backticks open and close a backtick span.
#[test]
fn advance_backtick_open_close() {
    let line = b"`a`";
    let mut s = ScanState::new();
    s.advance(line, 0); // '`'
    assert!(s.in_backtick);
    s.advance(line, 1); // 'a'
    assert!(s.in_backtick);
    s.advance(line, 2); // '`'
    assert!(!s.in_backtick);
}

// A `#` outside any scope is a comment boundary.
#[test]
fn boundary_hash_is_comment() {
    let s = ScanState::new();
    assert_eq!(boundary(b"#", 0, &s), Boundary::Comment);
}

// A `#` inside quotes is ordinary content.
#[test]
fn boundary_hash_in_quote_is_char() {
    let s = ScanState {
        in_quote: true,
        ..ScanState::new()
    };
    assert_eq!(boundary(b"#", 0, &s), Boundary::Char);
}

// A `#` inside a backtick span is ordinary content.
#[test]
fn boundary_hash_in_backtick_is_char() {
    let s = ScanState {
        in_backtick: true,
        ..ScanState::new()
    };
    assert_eq!(boundary(b"#", 0, &s), Boundary::Char);
}

// A `;` at top level is a separator.
#[test]
fn boundary_semicolon_is_separator() {
    let s = ScanState::new();
    assert_eq!(boundary(b";", 0, &s), Boundary::Separator);
}

// A `;` inside a `$( )` scope is not a separator.
#[test]
fn boundary_semicolon_in_dollar_paren_is_char() {
    let s = ScanState {
        dollar_paren_depth: 1,
        ..ScanState::new()
    };
    assert_eq!(boundary(b";", 0, &s), Boundary::Char);
}

// A `;` inside quotes is not a separator.
#[test]
fn boundary_semicolon_in_quote_is_char() {
    let s = ScanState {
        in_quote: true,
        ..ScanState::new()
    };
    assert_eq!(boundary(b";", 0, &s), Boundary::Char);
}

// A `;` inside a backtick span is not a separator.
#[test]
fn boundary_semicolon_in_backtick_is_char() {
    let s = ScanState {
        in_backtick: true,
        ..ScanState::new()
    };
    assert_eq!(boundary(b";", 0, &s), Boundary::Char);
}

// End of line is always a separator (when not scoped).
#[test]
fn boundary_eol_is_separator() {
    let s = ScanState::new();
    assert_eq!(boundary(b"ab", 2, &s), Boundary::Separator);
}

// A `#` mid-word is data once a word char has been consumed.
#[test]
fn boundary_hash_mid_word_is_char() {
    let line = b"a#";
    let mut s = ScanState::new();
    s.advance(line, 0); // 'a' sets word_active
    assert_eq!(boundary(line, 1, &s), Boundary::Char);
}

// A whitespace byte ends the current word.
#[test]
fn advance_space_resets_word_active() {
    let line = b"a ";
    let mut s = ScanState::new();
    s.advance(line, 0);
    assert!(s.word_active);
    s.advance(line, 1);
    assert!(!s.word_active);
}

// A `)` closing a `$( )` substitution keeps the word active (so `#` stays data).
#[test]
fn advance_substitution_close_keeps_word() {
    let line = b"$()";
    let mut s = ScanState::new();
    s.advance(line, 0); // `$( `
    s.advance(line, 2); // `)`
    assert!(s.word_active);
}

// A top-level (non-substitution) paren ends the current word.
#[test]
fn advance_top_level_parens_reset_word() {
    let mut s = ScanState::new();
    s.advance(b")", 0);
    assert!(!s.word_active, "a top-level `)` ends the word");
    let mut s = ScanState::new();
    s.advance(b"(", 0);
    assert!(!s.word_active, "a top-level `(` ends the word");
}

#[test]
fn is_word_break_chars() {
    let breaks = [b' ', b'\t', b'\n', b';', b'|', b'&', b'<', b'>'];
    for &b in &breaks {
        assert!(super::advance::is_word_break(b));
    }
    let words = [b'a', b'0', b'_', b'-', b'.', b'/', b'{', b'"', b'#'];
    for &b in &words {
        assert!(!super::advance::is_word_break(b));
    }
}

#[test]
fn skip_comment_advances_past_all_chars() {
    // Mutants: i += 1 → i -= 1 or i *= 1 would return wrong index
    let result = skip_comment(b"abc#def\nghi", 3);
    assert_eq!(result, 8); // # at index 3, \n at index 7, returns 7+1=8
}

#[test]
fn skip_comment_handles_no_newline() {
    // Mutants would fail to advance correctly
    let result = skip_comment(b"abc#def", 3);
    assert_eq!(result, 8); // returns len+1 (past slice end) when no newline found
}
