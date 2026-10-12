#![allow(clippy::indexing_slicing)]
use super::*;

// `$( )` is consumed together, advancing two positions and raising depth.
#[test]
fn advance_dollar_paren_advances_two() {
    let mut s = ScanState::new();
    let next = s.advance(b"$(x", 0);
    assert_eq!(next, 2, "$ ( must be consumed together");
    assert_eq!(s.paren_depth, 1);
}

// `$var` (no paren) advances one position and does not raise depth.
#[test]
fn advance_dollar_var_advances_one() {
    let mut s = ScanState::new();
    let next = s.advance(b"$x", 0);
    assert_eq!(next, 1);
    assert_eq!(s.paren_depth, 0);
}

// A top-level `((` pair is consumed together, advancing two positions and
// raising the paren depth (the `((…))` arithmetic-command opener).
#[test]
fn advance_double_paren_advances_two() {
    let mut s = ScanState::new();
    let next = s.advance(b"((x", 0);
    assert_eq!(next, 2, "( ( must be consumed together");
    assert_eq!(s.paren_depth, 1);
}

// A single top-level `(` does not raise the paren depth (ordinary paren).
#[test]
fn advance_single_paren_does_not_raise_depth() {
    let mut s = ScanState::new();
    let next = s.advance(b"(x", 0);
    assert_eq!(next, 1);
    assert_eq!(s.paren_depth, 0);
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
        paren_depth: 1,
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
    assert_eq!(result, 7); // returns len (slice end) when no newline found
}

// POSIX #4.1: an unquoted escape pair `\X` is consumed together, so the
// escaped byte never triggers a byte-level rule.
#[test]
fn escape_pair_does_not_break_the_word() {
    let mut s = ScanState::new();
    s.advance(b"a\\;b", 0);
    // The pair at index 1..3 is consumed together, landing on `b`.
    assert_eq!(s.advance(b"a\\;b", 1), 3);
    assert!(s.word_active, "the escaped `;` must not end the word");
}

#[test]
fn escaped_newline_is_a_line_continuation() {
    let mut s = ScanState::new();
    // The pair `\<newline>` is consumed together, so the newline is data.
    assert_eq!(s.advance(b"a\\\nb", 1), 3);
    assert!(s.word_active, "the escaped newline must not end the line");
}

#[test]
fn escaped_quote_does_not_toggle_quote_state() {
    let mut s = ScanState::new();
    assert_eq!(s.advance(b"a\\\"b", 1), 3);
    assert!(!s.in_quote, "the escaped quote is data, not a quote toggle");
}

#[test]
fn trailing_backslash_clamps_to_the_line_end() {
    let mut s = ScanState::new();
    // The jump clamps to `line.len()` so the caller's end-of-line boundary
    // still fires and flushes the word.
    assert_eq!(s.advance(b"a\\", 1), 2);
    assert_eq!(s.advance(b"\\", 0), 1);
}

#[test]
fn escaped_paren_never_nests_a_substitution() {
    let mut s = ScanState::new();
    let line = b"$(";
    s.advance(line, 0);
    assert_eq!(s.paren_depth, 1);
    // `$(a\)b)` — the escaped `)` is data, so only the real `)` closes.
    let line = b"$(a\\)b)";
    assert_eq!(s.advance(line, 2), 3, "the word byte advances one");
    assert_eq!(s.advance(line, 3), 5, "the pair `\\)` is consumed together");
    assert_eq!(s.paren_depth, 1, "the escaped `)` must not close the body");
    s.advance(line, 5);
    s.advance(line, 6);
    assert_eq!(s.paren_depth, 0, "the unescaped `)` closes the body");
}

// Task #130: inside double quotes a nested region is consumed whole, so its
// `;`, `|`, `&&`, `<`/`>` and `"` are never seen by the byte-level scanners.
#[test]
fn quoted_dollar_paren_region_is_skipped_whole() {
    let line = b"echo \"$(echo \"a; b\")\"";
    let mut s = ScanState::new();
    assert_eq!(s.advance(line, 5), 6, "the opening quote toggles");
    assert!(s.in_quote);
    assert_eq!(s.advance(line, 6), 20, "the `$(` jumps past the `)` at 19");
    assert!(s.in_quote, "the outer quote stays open across the region");
    assert_eq!(s.paren_depth, 0, "the region is not nested, it is consumed");
    assert!(s.word_active, "the region is word content");
    assert_eq!(s.advance(line, 20), 21);
    assert!(!s.in_quote);
}

#[test]
fn quoted_brace_region_is_skipped_whole() {
    let line = b"echo \"${u:-\"a; b\"}\"";
    let mut s = ScanState::new();
    s.advance(line, 5);
    assert_eq!(s.advance(line, 6), 18, "past the closing brace at 17");
    assert!(s.in_quote);
    assert_eq!(s.advance(line, 18), 19);
    assert!(!s.in_quote);
}

#[test]
fn quoted_backtick_region_is_skipped_whole() {
    let line = b"echo \"`printf \"q;\"`\"";
    let mut s = ScanState::new();
    s.advance(line, 5);
    assert_eq!(s.advance(line, 6), 19, "past the closing backtick at 18");
    assert!(s.in_quote);
    assert!(!s.in_backtick, "the region is consumed, not entered");
    assert_eq!(s.advance(line, 19), 20);
    assert!(!s.in_quote);
}

// An unbalanced region is not a skip: the scanner falls back to the
// byte-at-a-time path, exactly as before task #130.
#[test]
fn unbalanced_quoted_region_falls_back_to_byte_scanning() {
    let mut s = ScanState::new();
    s.advance(b"echo \"$(echo a\"", 5);
    assert_eq!(s.advance(b"echo \"$(echo a\"", 6), 7, "unclosed `$( … )`");
    let mut s = ScanState::new();
    s.advance(b"echo \"${x", 5);
    assert_eq!(s.advance(b"echo \"${x", 6), 7, "unclosed brace expansion");
    let mut s = ScanState::new();
    s.advance(b"echo \"`a", 5);
    assert_eq!(s.advance(b"echo \"`a", 6), 7, "unclosed backtick region");
}

// The unquoted path is untouched: a `$(` outside quotes still raises the depth.
#[test]
fn unquoted_regions_keep_the_incremental_tracking() {
    let mut s = ScanState::new();
    assert_eq!(s.advance(b"echo $(x)", 5), 7);
    assert_eq!(s.paren_depth, 1);
    assert_eq!(s.advance(b"echo $(x)", 8), 9);
    assert_eq!(s.paren_depth, 0);
}

// A full walk of each evidence line ends balanced: no open quote, no open
// substitution, so statement splitting and `is_bare` see a complete word.
#[test]
fn full_walk_of_quoted_regions_ends_balanced() {
    for line in [
        b"echo \"$(echo \"a; b\")\"".as_slice(),
        b"echo \"${u:-\"a; b\"}\"".as_slice(),
        b"echo \"`printf \"q;\"`\"".as_slice(),
        b"echo \"$(echo \"$(echo deep)\")\"".as_slice(),
        b"echo \"$(printf \"x && y\")\" && echo ok".as_slice(),
        b"echo \"$(printf \"a | b\")\"".as_slice(),
        b"echo \"$(printf \"<<EOF\")\"".as_slice(),
    ] {
        let mut s = ScanState::new();
        let mut i = 0usize;
        while i < line.len() {
            i = s.advance(line, i);
        }
        assert!(!s.in_quote, "{line:?}: the quote closes");
        assert!(!s.in_backtick, "{line:?}: no backtick is left open");
        assert_eq!(s.paren_depth, 0, "{line:?}: no substitution is left open");
    }
}

// --- Single quotes (task #141): a `'…'` span is one opaque byte run.

// A `'` opens the span and its closing `'` ends it; the bytes between are data.
#[test]
fn advance_single_quote_span_open_close() {
    let line = b"a'b'c";
    let mut s = ScanState::new();
    s.advance(line, 0); // 'a'
    assert!(!s.in_single);
    s.advance(line, 1); // '\''
    assert!(s.in_single);
    s.advance(line, 2); // 'b'
    assert!(s.in_single, "the span byte advances one position");
    s.advance(line, 3); // '\''
    assert!(!s.in_single);
}

// Inside a span a backslash is literal (POSIX §2.2.3): it consumes one byte
// and shields nothing, so the `'` behind it closes the span.
#[test]
fn advance_span_has_no_escape_pair() {
    let mut s = ScanState::new();
    s.advance(b"'\\", 0); // opens the span
    assert!(s.in_single);
    let next = s.advance(b"'\\", 1); // the `\` is one data byte
    assert_eq!(next, 2, "no escape pair inside a span: one byte");
    assert!(s.in_single);
}

// A `#` inside a `'…'` span is data, never a comment.
#[test]
fn boundary_hash_in_single_quote_is_char() {
    let s = ScanState {
        in_single: true,
        ..ScanState::new()
    };
    assert_eq!(
        boundary(b"a'#b'", 2, &s),
        Boundary::Char,
        "the span's `#` is not a comment"
    );
}

// A `;` inside a `'…'` span is not a separator (`echo 'a; b'` is one command).
#[test]
fn boundary_semicolon_in_single_quote_is_char() {
    let s = ScanState {
        in_single: true,
        ..ScanState::new()
    };
    assert_eq!(boundary(b"a;b", 1, &s), Boundary::Char);
}

// A `'` is a word byte, and mid-span no new word can start.
#[test]
fn at_word_start_is_false_inside_a_span() {
    let mut s = ScanState::new();
    s.advance(b"'x", 0);
    assert!(s.in_single);
    assert!(!s.at_word_start(), "mid-span is never a word start");
}

// A `"` written inside a span does not toggle the double-quote state.
#[test]
fn advance_quote_inside_span_does_not_toggle() {
    let mut s = ScanState::new();
    s.advance(b"'\"", 0);
    assert!(s.in_single);
    s.advance(b"'\"", 1);
    assert!(s.in_single, "the span is still open");
    assert!(!s.in_quote, "the inner quote byte is data");
}

// The `Quotes` carrier hands *both* quote bits across a nested scan, and
// `set_quotes` must restore both: a group/function-block scan that resumed with
// only the `"` half would read the rest of a `'…'` span as bare bytes (task #141).
#[test]
fn set_quotes_restores_both_bits() {
    let mut s = ScanState::new();
    s.set_quotes(Quotes {
        double: true,
        single: true,
    });
    assert!(s.in_quote, "the double bit is restored");
    assert!(s.in_single, "the single bit is restored");
    assert_eq!(
        (s.quotes().double, s.quotes().single),
        (true, true),
        "quotes() round-trips both fields"
    );
    // The carrier seeds a fresh state the same way (the asymmetric pair proves
    // neither field is ignored).
    let seeded = Quotes {
        double: false,
        single: true,
    }
    .scan_state();
    assert!(
        !seeded.in_quote && seeded.in_single,
        "only the span bit is set, not the double bit"
    );
}
