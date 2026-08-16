#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

struct R {
    end: usize,
    closed: bool,
    quote: bool,
    start: usize,
}

fn scan(line: &[u8], start: usize) -> R {
    let mut in_quote = false;
    let mut s = start;
    let (end, closed) = scan_block(line, start, &mut in_quote, &mut s, 1);
    R {
        end,
        closed,
        quote: in_quote,
        start: s,
    }
}

// A comment that contains a closing keyword must not close the block.
#[test]
fn comment_containing_closer_does_not_close() {
    let r = scan(b"if a; then b; # fi", 2);
    assert!(!r.closed, "fi inside # comment must not close the block");
    assert_eq!(r.end, 19);
}

// A single (unclosed) quote must leave in_quote set.
#[test]
fn unclosed_quote_leaves_in_quote() {
    let r = scan(b"if \"a; b; fi", 2);
    assert!(r.quote, "odd number of quotes must leave in_quote true");
    assert!(r.closed);
}

// `$var` (no paren) must not open a dollar-paren scope.
#[test]
fn dollar_var_is_not_dollar_paren() {
    let r = scan(b"if $x; then y; fi; echo z", 2);
    assert_eq!(r.end, 18, "fi must close at its own ';'");
    assert!(r.closed);
}

// `$( )` must open and close a dollar-paren scope.
#[test]
fn dollar_paren_single() {
    let r = scan(b"if $(a); then y; fi; echo z", 2);
    assert_eq!(r.end, 20);
    assert!(r.closed);
}

// Nested `$( $( ) )` must track depth.
#[test]
fn dollar_paren_nested() {
    let r = scan(b"if $(a $(b); c); then y; fi; echo z", 2);
    assert_eq!(r.end, 28);
    assert!(r.closed);
}

// A plain top-level `(` is not a dollar-paren scope.
#[test]
fn plain_top_level_paren() {
    let r = scan(b"if (a; b); then y; fi; echo z", 2);
    assert_eq!(r.end, 22);
    assert!(r.closed);
}

// Backticks form a scope; ';' inside is not a separator.
#[test]
fn backtick_scope() {
    let r = scan(b"if `a; b`; then y; fi; echo z", 2);
    assert_eq!(r.end, 22);
    assert!(r.closed);
}

// A double-quoted string with ';' inside is not a separator.
#[test]
fn double_quote_scope() {
    let r = scan(b"if \"a; b\"; then y; fi; echo z", 2);
    assert_eq!(r.end, 22);
    assert!(r.closed);
}

// `;` inside `$( )` is not a top-level separator.
#[test]
fn semicolon_inside_dollar_paren() {
    let r = scan(b"if $(a; b); then y; fi; echo z", 2);
    assert_eq!(r.end, 23);
    assert!(r.closed);
}

// `$var` inside double quotes must not open a dollar-paren scope.
#[test]
fn dollar_var_inside_quotes() {
    let r = scan(b"if \"$x\"; then y; fi; echo z", 2);
    assert_eq!(r.end, 20);
    assert!(r.closed);
}

// `$( )` inside backticks.
#[test]
fn dollar_paren_inside_backtick() {
    let r = scan(b"if `$(a)`; then y; fi; echo z", 2);
    assert_eq!(r.end, 22);
    assert!(r.closed);
}

// A stray `)` at top level is not a dollar-paren closer.
#[test]
fn stray_top_level_closing_paren() {
    let r = scan(b"if a); then y; fi; echo z", 2);
    assert_eq!(r.end, 18);
    assert!(r.closed);
}

// `fi` not followed by a separator must not close the block early.
#[test]
fn closer_without_separator() {
    let r = scan(b"if $(a); then y; fi z; echo w", 2);
    assert_eq!(r.end, 22);
    assert!(r.closed);
}

// Space between `$( )` and the next separator.
#[test]
fn space_after_dollar_paren() {
    let r = scan(b"if $(a) ; then y; fi", 2);
    assert_eq!(r.end, 21);
    assert!(r.closed);
}

// `$( )` inside double quotes is a scope.
#[test]
fn dollar_paren_inside_double_quote() {
    let r = scan(b"if \"a$(b; c)\"; then y; fi; echo z", 2);
    assert_eq!(r.end, 26);
    assert!(r.closed);
}

// Nested `$( )` without a semicolon inside.
#[test]
fn dollar_paren_nested_no_semicolon() {
    let r = scan(b"if $(a $(b)); then y; fi; echo z", 2);
    assert_eq!(r.end, 25);
    assert!(r.closed);
}

// Backtick scope followed by more content on the same line.
#[test]
fn backtick_scope_trailing_content() {
    let r = scan(b"if `a` b; then y; fi; echo z", 2);
    assert_eq!(r.end, 21);
    assert!(r.closed);
}

// Double-quote scope followed by more content on the same line.
#[test]
fn double_quote_scope_trailing_content() {
    let r = scan(b"if \"a\" b; then y; fi; echo z", 2);
    assert_eq!(r.end, 21);
    assert!(r.closed);
}

// A closing keyword inside `$( )` must not close the block.
#[test]
fn closer_inside_dollar_paren() {
    let r = scan(b"if $(a; fi); then y; fi; echo z", 2);
    assert_eq!(r.end, 24);
    assert!(r.closed);
}

// A comment spanning to end-of-line before the real closer.
#[test]
fn comment_before_newline_then_closer() {
    let r = scan(b"if a # c\nthen y; fi", 2);
    assert_eq!(r.end, 20);
    assert!(r.closed);
}

// A space-delimited closer inside `$( )` is a real closer.
#[test]
fn space_delimited_closer_inside_dollar_paren() {
    let r = scan(b"if $( fi ; a); then y; fi; echo z", 2);
    assert_eq!(r.end, 14, "clean fi token must close at the inner ';'");
}

// A `(` inside double quotes (within a `$( )`) is not a plain paren.
#[test]
fn paren_inside_double_quote_within_dollar_paren() {
    let r = scan(b"if $(x \"a(b\"); then y; fi; echo z", 2);
    assert_eq!(r.end, 26);
}

// A `(` inside backticks (within a `$( )`) is not a plain paren.
#[test]
fn paren_inside_backtick_within_dollar_paren() {
    let r = scan(b"if $(x `a(b`); then y; fi; echo z", 2);
    assert_eq!(r.end, 26);
}

// An unmatched top-level `(` does not open a dollar-paren scope.
#[test]
fn unmatched_top_level_paren() {
    let r = scan(b"if (a; b; then y; fi; echo z", 2);
    assert_eq!(r.end, 21);
}

// A balanced top-level `(...)` mid-line does not open a dollar-paren scope.
#[test]
fn balanced_top_level_paren_mid_line() {
    let r = scan(b"if a (b; c) d; then y; fi; echo z", 2);
    assert_eq!(r.end, 26);
}

// Non-paren content inside `$( )` does not close the scope early.
#[test]
fn content_inside_dollar_paren() {
    let r = scan(b"if $(a b); then y; fi; echo z", 2);
    assert_eq!(r.end, 22);
}

// Backtick scope with a space inside.
#[test]
fn backtick_scope_with_space() {
    let r = scan(b"if `a b` c; then y; fi; echo z", 2);
    assert_eq!(r.end, 23);
}

// A backtick inside double quotes is not a backtick scope.
#[test]
fn backtick_inside_double_quote() {
    let r = scan(b"if \"a`b\"; then y; fi; echo z", 2);
    assert_eq!(r.end, 21);
}

// Two adjacent backtick scopes.
#[test]
fn adjacent_backtick_scopes() {
    let r = scan(b"if `a` `b`; then y; fi; echo z", 2);
    assert_eq!(r.end, 23);
}

// `$( )` immediately followed by a plain paren.
#[test]
fn dollar_paren_then_plain_paren() {
    let r = scan(b"if $(a) (b; c); then y; fi; echo z", 2);
    assert_eq!(r.end, 27);
}

// Nested `$( )` with content and no spaces.
#[test]
fn nested_dollar_paren_compact() {
    let r = scan(b"if $(a$(b);c); then y; fi; echo z", 2);
    assert_eq!(r.end, 26);
}

// A `(` inside double quotes at top level.
#[test]
fn paren_inside_double_quote_top_level() {
    let r = scan(b"if \"a(b\"; then y; fi; echo z", 2);
    assert_eq!(r.end, 21);
}

// A `(` inside backticks at top level.
#[test]
fn paren_inside_backtick_top_level() {
    let r = scan(b"if `a(b`; then y; fi; echo z", 2);
    assert_eq!(r.end, 21);
}

// A plain `(` inside `$( )` adds a depth level, so the following `;` is not
// a top-level separator and the closer is only counted at end of line.
#[test]
fn plain_paren_inside_dollar_paren_adds_depth() {
    let r = scan(b"if $(x (a) fi;", 2);
    assert_eq!(r.end, 15, "plain ( ) must raise dollar-paren depth");
}
