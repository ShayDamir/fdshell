#![allow(clippy::unwrap_used)]

use alloc::vec;
use alloc::vec::Vec;

fn m(pat: &[u8], name: &[u8]) -> bool {
    super::match_component(pat, &[], name, false)
}

fn m_masked(pat: &[u8], mask: &[bool], name: &[u8]) -> bool {
    super::match_component(pat, mask, name, false)
}

/// `match_component` with `dotglob` on.
fn m_dot(pat: &[u8], name: &[u8]) -> bool {
    super::match_component(pat, &[], name, true)
}

#[test]
fn star_matches_any_run() {
    assert!(m(b"*", b""));
    assert!(m(b"*", b"anything"));
    assert!(m(b"a*", b"a"));
    assert!(m(b"a*", b"apple"));
    assert!(!m(b"a*", b"b"));
    assert!(m(b"*a", b"a"));
    assert!(m(b"*a", b"banana"));
    assert!(!m(b"*a", b"banan"));
    assert!(m(b"*a*", b"xax"));
    assert!(m(b"*a*", b"a"));
    assert!(!m(b"*a*", b"b"));
}

#[test]
fn double_star_is_not_recursive() {
    // v1: `**` behaves exactly like `*` (one component, no `/` crossing).
    assert!(m(b"**", b"any name"));
    assert!(m(b"a**", b"ax"));
    assert!(m(b"**z", b"zz"));
    assert!(!m(b"a**", b"bz"));
}

#[test]
fn question_matches_exactly_one_byte() {
    assert!(m(b"?", b"a"));
    assert!(!m(b"?", b""));
    assert!(!m(b"?", b"ab"));
    assert!(m(b"a?c", b"aXc"));
    assert!(!m(b"a?c", b"aXb"));
}

#[test]
fn unquoted_backslash_escapes_the_next_byte() {
    assert!(m(b"a\\*", b"a*"));
    assert!(!m(b"a\\*", b"ax"));
    assert!(m(b"a\\?", b"a?"));
    // A lone trailing backslash is a literal backslash.
    assert!(m(b"\\", b"\\"));
    assert!(!m(b"\\", b"x"));
}

#[test]
fn quoted_bytes_are_always_literal() {
    // `a"*"` (mask bit 1 set) matches only the literal name `a*`.
    assert!(m_masked(b"a*", &[false, true], b"a*"));
    assert!(!m_masked(b"a*", &[false, true], b"ax"));
    // Quoted `?` and `[` are literal too.
    assert!(m_masked(b"?", &[true], b"?"));
    assert!(!m_masked(b"?", &[true], b"z"));
    assert!(m_masked(b"[a", &[false, true], b"[a"));
}

#[test]
fn bracket_class_membership() {
    assert!(m(b"[ab]", b"a"));
    assert!(m(b"[ab]", b"b"));
    assert!(!m(b"[ab]", b"c"));
    assert!(m(b"x[ab]z", b"xbz"));
    assert!(!m(b"x[ab]z", b"xzz"));
}

#[test]
fn bracket_negation() {
    assert!(m(b"[!a]", b"b"));
    assert!(!m(b"[!a]", b"a"));
    assert!(m(b"[!a-z]1", b"21"));
    assert!(!m(b"[!a-z]1", b"a1"));
}

#[test]
fn bracket_ranges() {
    assert!(m(b"[a-c]", b"b"));
    assert!(!m(b"[a-c]", b"d"));
    assert!(m(b"[a-c]-[a-c]", b"a-c"));
    assert!(!m(b"[a-c]-[a-c]", b"a-d"));
    // A `-` as the last member is literal, not a range.
    assert!(m(b"[a-]", b"-"));
    assert!(m(b"[a-]", b"a"));
    assert!(!m(b"[a-]", b"b"));
}

#[test]
fn bracket_leading_close_is_literal() {
    assert!(m(b"[]a]", b"]"));
    assert!(m(b"[]a]", b"a"));
    assert!(!m(b"[]a]", b"b"));
    assert!(m(b"[!]x]", b"!"));
    assert!(!m(b"[!]x]", b"x"));
}

#[test]
fn bracket_invalid_range_matches_nothing_in_its_span() {
    // `[z-a]`: invalid range before the closing `]` stops the scan — the
    // class is empty and matches nothing.
    assert!(!m(b"[z-a]", b"m"));
    assert!(!m(b"[z-a]", b"z"));
    // `[z-ax]`: invalid range skipped, `x` is the next member.
    assert!(m(b"[z-ax]", b"x"));
    assert!(!m(b"[z-ax]", b"m"));
    // Negation of an empty class matches everything.
    assert!(m(b"[!z-a]", b"m"));
}

#[test]
fn bracket_quoted_bytes_are_literal_members() {
    // A quoted `]` is a class member; the next unquoted `]` closes.
    assert!(m_masked(b"[]]]", &[false, true, false, false], b"]]"));
    assert!(!m_masked(b"[]]]", &[false, true, false, false], b"]"));
    assert!(!m_masked(b"[]]]", &[false, true, false, false], b"a]"));
    // The first `]` after `[` is a member even when quoted.
    assert!(m_masked(b"[]a]", &[false, true, false, false], b"]"));
    assert!(m_masked(b"[]a]", &[false, true, false, false], b"a"));
    assert!(!m_masked(b"[]a]", &[false, true, false, false], b"b"));
}

#[test]
fn bracket_backslash_escapes_are_literal_members() {
    // An unquoted `\X` inside a class is an escaped literal member: `\]`
    // does not close the class, the next unquoted `]` does.
    assert!(m(b"[\\]]x", b"]x"));
    assert!(!m(b"[\\]]x", b"x"));
    assert!(!m(b"[\\]]", b"a"));
    // An escaped `]` mid-class: the member is `]`, later bytes still count.
    assert!(m(b"[\\]a]", b"a"));
    assert!(m(b"[\\]a]", b"]"));
    assert!(!m(b"[\\]a]", b"b"));
    // A quoted backslash is a plain member, not an escape: the next
    // unquoted `]` closes.
    assert!(m_masked(
        b"[\\]]x",
        &[false, true, false, false, false],
        b"\\]x"
    ));
    assert!(!m_masked(
        b"[\\]]x",
        &[false, true, false, false, false],
        b"]x"
    ));
    assert!(m_masked(b"[\\]x", &[false, true, false, false], b"\\x"));
    assert!(!m_masked(b"[\\]x", &[false, true, false, false], b"]x"));
}

#[test]
fn bracket_quoted_close_continues_the_class() {
    // A quoted `]` mid-class is a member; the scan continues past it and
    // the unquoted `]` closes.
    assert!(m_masked(
        b"[ab]c]",
        &[false, false, false, true, false, false],
        b"c"
    ));
    assert!(m_masked(
        b"[ab]c]",
        &[false, false, false, true, false, false],
        b"]"
    ));
    assert!(!m_masked(
        b"[ab]c]",
        &[false, false, false, true, false, false],
        b"d"
    ));
    // The class ends at the unquoted `]`: later pattern bytes are literals,
    // not members.
    assert!(!m(b"[ab]x", b"]x"));
    assert!(m(b"[ab]x", b"ax"));
}

#[test]
fn bracket_range_boundaries_and_continuation() {
    // A single-byte range `[a-a]` is a valid member, not an invalid range.
    assert!(m(b"[a-a]", b"a"));
    assert!(!m(b"[a-a]", b"b"));
    // The byte right after a range end is a regular member.
    assert!(m(b"[a-ce]", b"e"));
    assert!(m(b"[a-ce]", b"b"));
    assert!(!m(b"[a-ce]", b"d"));
    // A dash right after a range end is a literal member: no chained range.
    assert!(m(b"[a-c-ef]", b"-"));
    assert!(m(b"[a-c-ef]", b"e"));
    assert!(m(b"[a-c-ef]", b"f"));
    assert!(!m(b"[a-c-ef]", b"d"));
}

#[test]
fn dot_rule_blocks_leading_bracket_class() {
    // A leading `[...]` that can contain `.` still cannot consume a name's
    // leading dot (FNM_PERIOD); the same class matches dot-less names.
    assert!(!m(b"[.a]x", b".x"));
    assert!(m(b"[.a]x", b"ax"));
    assert!(!m(b"[.]a", b".a"));
}

#[test]
fn unclosed_bracket_is_literal() {
    assert!(m(b"[abc", b"[abc"));
    assert!(!m(b"[abc", b"a"));
}

#[test]
fn dot_rule_blocks_pattern_leads_on_dot_names() {
    assert!(!m(b"*", b".hidden"));
    assert!(!m(b"?", b"."));
    assert!(!m(b"[ab]*", b".x"));
    // A literal leading byte may still match the dot.
    assert!(m(b".*", b".hidden"));
    assert!(m(b"?.", b"x."));
    assert!(!m(b"?.", b"ab"));
    // Non-dot names are unaffected.
    assert!(m(b"*", b"visible"));
}

#[test]
fn dotglob_allows_leading_pattern_bytes_on_dot_names() {
    // With dotglob, a leading unquoted `*`/`?` may eat the dot.
    assert!(m_dot(b"*", b".hidden"));
    assert!(m_dot(b"?", b"."));
    // A leading `[...]` may consume the dot only if it contains it: `[ab]`
    // cannot match `.`, so `[ab]*` still does not match `.x`; `[.a]*` does.
    assert!(!m_dot(b"[ab]*", b".x"));
    assert!(m_dot(b"[.a]*", b".x"));
    // A literal leading byte still matches (as before).
    assert!(m_dot(b".*", b".hidden"));
    // dotglob does not affect non-dot names.
    assert!(m_dot(b"*", b"visible"));
}

#[test]
fn dotglob_does_not_match_dot_or_dotdot_via_bare_star() {
    // Even with dotglob, bare `*`/`?` must not match `.`/`..` (the walk
    // gates those on a literal `.` first byte); the matcher alone blocks them
    // via the `.`/`..` names being two-byte / the star crossing nothing.
    assert!(!m_dot(b"?", b".."));
}

#[test]
fn dotglob_quoted_leading_byte_still_literal() {
    // A quoted leading `*` is literal regardless of dotglob.
    assert!(!m_masked(b"*", &[true], b".x"));
    assert!(m_masked(b"*", &[true], b"*"));
}

#[test]
fn posix_class_membership() {
    assert!(m(b"[[:alpha:]]", b"a"));
    assert!(m(b"[[:alpha:]]", b"Z"));
    assert!(!m(b"[[:alpha:]]", b"5"));
    assert!(m(b"[[:digit:]]", b"9"));
    assert!(!m(b"[[:digit:]]", b"a"));
    // Negation of a class.
    assert!(m(b"[![:space:]]", b"x"));
    assert!(!m(b"[![:space:]]", b" "));
    // A class mixed with a range and a literal.
    assert!(m(b"[[:digit:]a-cc]", b"b"));
    assert!(m(b"[[:digit:]a-cc]", b"7"));
    assert!(!m(b"[[:digit:]a-cc]", b"d"));
    // An unknown class is literal members, not a class.
    assert!(m(b"[[:bogus:]]", b":"));
    assert!(m(b"[[:bogus:]]", b"b"));
    assert!(!m(b"[[:bogus:]]", b"5"));
}

#[test]
fn posix_class_quoted_bracket_is_literal() {
    // A quoted `[` does not open a class: the whole word is one literal.
    let mask = [true, false, false, false, false, false, false, false, false];
    assert!(m_masked(b"[:alpha:]]", &mask, b"[:alpha:]]"));
    assert!(!m_masked(b"[:alpha:]]", &mask, b"a"));
}

#[test]
fn has_unquoted_pattern_table() {
    assert!(super::has_unquoted_pattern(b"a*", &[]));
    assert!(super::has_unquoted_pattern(b"?", &[]));
    assert!(super::has_unquoted_pattern(b"[ab]", &[]));
    assert!(super::has_unquoted_pattern(b"x[y]z", &[]));
    assert!(!super::has_unquoted_pattern(b"plain", &[]));
    assert!(!super::has_unquoted_pattern(b"", &[]));
    assert!(!super::has_unquoted_pattern(b"[abc", &[]));
    assert!(!super::has_unquoted_pattern(b"a\\*", &[]));
    assert!(!super::has_unquoted_pattern(b"\\[", &[]));
    assert!(!super::has_unquoted_pattern(b"a*", &[false, true]));
    assert!(super::has_unquoted_pattern(b"a*b", &[false, false, false]));
    assert!(!super::has_unquoted_pattern(b"a*b", &[false, true, false]));
}

#[test]
fn ten_k_char_literal_and_star_does_not_overflow() {
    let lit: Vec<u8> = core::iter::repeat_n(b'a', 10_000).collect();
    // A 10K all-literal pattern must scan linearly (no recursion).
    assert!(m(&lit, &lit));
    // Longer name: the pattern exhausts first and there is no star to eat the rest.
    let mut name = lit.clone();
    name.push(b'b');
    assert!(!m(&lit, &name));
    // A 10K literal that fails on the final byte must not recurse either.
    let mut short = lit.clone();
    short.pop();
    assert!(!m(&lit, &short));
    let star: Vec<u8> = core::iter::repeat_n(b'*', 10_000).collect();
    let long: Vec<u8> = core::iter::repeat_n(b'z', 10_000).collect();
    assert!(m(&star, &long));
    assert!(m(&star, b""));
}

#[test]
fn heavy_star_backtracking_stays_finite() {
    // `*a*a*...a` (500 stars) vs a long non-matching tail: quadratic but
    // iterative — must finish without a stack overflow.
    let mut pat = vec![b'*'];
    for _ in 0..500 {
        pat.extend_from_slice(b"a*");
    }
    let name: Vec<u8> = core::iter::repeat_n(b'a', 1000).collect();
    assert!(m(&pat, &name));
    pat.push(b'c');
    assert!(!m(&pat, &name));
}
