#![allow(clippy::unwrap_used)]

use crate::mask::MaskBit;
use crate::mask::MaskBit::{Quoted, Unquoted};
use alloc::vec;
use alloc::vec::Vec;

fn m(pat: &[u8], name: &[u8]) -> bool {
    super::match_component(pat, &[], name, false)
}

fn m_masked(pat: &[u8], mask: &[MaskBit], name: &[u8]) -> bool {
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
    assert!(m_masked(b"a*", &[Unquoted, Quoted], b"a*"));
    assert!(!m_masked(b"a*", &[Unquoted, Quoted], b"ax"));
    // Quoted `?` and `[` are literal too.
    assert!(m_masked(b"?", &[Quoted], b"?"));
    assert!(!m_masked(b"?", &[Quoted], b"z"));
    assert!(m_masked(b"[a", &[Unquoted, Quoted], b"[a"));
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
    // bash accepts `^` in the same slot as `!` (POSIX defines only `!`).
    assert!(m(b"[^a]", b"b"));
    assert!(!m(b"[^a]", b"a"));
    assert!(m(b"[^a-z]1", b"21"));
    assert!(!m(b"[^a-z]1", b"a1"));
}

/// bash's `^` marker counts only in the marker's slot: the quote mask gates
/// it, so a quoted `^`, an escaped `\^` and a mid-bracket `^` are plain
/// members, and `[^]` is simply an unclosed bracket.
#[test]
fn bracket_caret_negation_is_mask_aware() {
    // A quoted `^` (bit 1 set, the `[` stays unquoted) is a literal member:
    // no negation, so `b` stays outside the class.
    let mask = [Unquoted, Quoted, Unquoted, Unquoted];
    assert!(m_masked(b"[^a]", &mask, b"^"));
    assert!(m_masked(b"[^a]", &mask, b"a"));
    assert!(!m_masked(b"[^a]", &mask, b"b"));
    // A token-text escape pair: `\^` is an escaped member, not a marker.
    assert!(m(b"[\\^a]", b"^"));
    assert!(m(b"[\\^a]", b"a"));
    assert!(!m(b"[\\^a]", b"b"));
    // `^` is a marker only right after `[`; mid-bracket it is a member.
    assert!(m(b"[a^]", b"^"));
    assert!(!m(b"[a^]", b"b"));
    // Negation plus a `^` member.
    assert!(m(b"[^^]", b"b"));
    assert!(!m(b"[^^]", b"^"));
    // `[^]` is unclosed (the `]` is the first member): the `[` is literal.
    assert!(m(b"[^]", b"[^]"));
    assert!(!m(b"[^]", b"b"));
    // A leading negated bracket keeps the pathname dot rule; under dotglob
    // (word patterns) it takes the dot only when it can match it.
    assert!(!m(b"[^x]b", b".b"));
    assert!(m_dot(b"[^x]b", b".b"));
    assert!(!m_dot(b"[^.x]b", b".b"));
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
    // Same rule behind a `^` marker: the first `]` is a member.
    assert!(m(b"[^]x]", b"!"));
    assert!(!m(b"[^]x]", b"x"));
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
    assert!(m(b"[^z-a]", b"m"));
}

#[test]
fn bracket_quoted_bytes_are_literal_members() {
    // A quoted `]` is a class member; the next unquoted `]` closes.
    assert!(m_masked(
        b"[]]]",
        &[Unquoted, Quoted, Unquoted, Unquoted],
        b"]]"
    ));
    assert!(!m_masked(
        b"[]]]",
        &[Unquoted, Quoted, Unquoted, Unquoted],
        b"]"
    ));
    assert!(!m_masked(
        b"[]]]",
        &[Unquoted, Quoted, Unquoted, Unquoted],
        b"a]"
    ));
    // The first `]` after `[` is a member even when quoted.
    assert!(m_masked(
        b"[]a]",
        &[Unquoted, Quoted, Unquoted, Unquoted],
        b"]"
    ));
    assert!(m_masked(
        b"[]a]",
        &[Unquoted, Quoted, Unquoted, Unquoted],
        b"a"
    ));
    assert!(!m_masked(
        b"[]a]",
        &[Unquoted, Quoted, Unquoted, Unquoted],
        b"b"
    ));
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
        &[Unquoted, Quoted, Unquoted, Unquoted, Unquoted],
        b"\\]x"
    ));
    assert!(!m_masked(
        b"[\\]]x",
        &[Unquoted, Quoted, Unquoted, Unquoted, Unquoted],
        b"]x"
    ));
    assert!(m_masked(
        b"[\\]x",
        &[Unquoted, Quoted, Unquoted, Unquoted],
        b"\\x"
    ));
    assert!(!m_masked(
        b"[\\]x",
        &[Unquoted, Quoted, Unquoted, Unquoted],
        b"]x"
    ));
}

#[test]
fn bracket_quoted_close_continues_the_class() {
    // A quoted `]` mid-class is a member; the scan continues past it and
    // the unquoted `]` closes.
    assert!(m_masked(
        b"[ab]c]",
        &[Unquoted, Unquoted, Unquoted, Quoted, Unquoted, Unquoted],
        b"c"
    ));
    assert!(m_masked(
        b"[ab]c]",
        &[Unquoted, Unquoted, Unquoted, Quoted, Unquoted, Unquoted],
        b"]"
    ));
    assert!(!m_masked(
        b"[ab]c]",
        &[Unquoted, Unquoted, Unquoted, Quoted, Unquoted, Unquoted],
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
    // The walk never lists `.`/`..` (for any pattern), so the matcher is
    // only asked about real names; a single `?` still cannot eat two bytes.
    assert!(!m_dot(b"?", b".."));
}

#[test]
fn dotglob_quoted_leading_byte_still_literal() {
    // A quoted leading `*` is literal regardless of dotglob.
    assert!(!m_masked(b"*", &[Quoted], b".x"));
    assert!(m_masked(b"*", &[Quoted], b"*"));
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
fn class_as_last_member_stops_at_the_closing_bracket() {
    // A class that is the bracket's last member must not leak the member
    // scan past the closing `]` (which would claim `]` and trailing pattern
    // bytes as members).
    assert!(m(b"[[:digit:]]x", b"5x"));
    assert!(!m(b"[[:digit:]]x", b"]x"));
    assert!(!m(b"[[:digit:]]xy", b"xxy"));
    // A member after the class keeps the scan inside the bracket.
    assert!(m(b"[[:digit:]x]y", b"xy"));
    assert!(m(b"[[:digit:]x]y", b"5y"));
    assert!(!m(b"[[:digit:]x]y", b"]y"));
}

#[test]
fn class_then_range_keeps_the_cursor_one_past_the_class() {
    // The class arm resumes the member scan with `c = bytes[close]` and
    // `i = close + 1`, keeping the loop's `c == bytes[i - 1]` invariant.
    // Resuming at `close` re-reads the byte after the class as a member whose
    // dash follows it, so a class followed by a range turns that byte into a
    // range **start**: `-z` would become `45..=122` and accept `5`, where the
    // real member list is the class, a literal `-` and a literal `z`.
    assert!(m(b"[[:alpha:]-z]", b"a")); // the class
    assert!(m(b"[[:alpha:]-z]", b"m")); // the class
    assert!(m(b"[[:alpha:]-z]", b"-")); // literal member
    assert!(m(b"[[:alpha:]-z]", b"z")); // literal member
    assert!(!m(b"[[:alpha:]-z]", b"5")); // not a range
}

#[test]
fn a_non_class_bracket_does_not_open_a_class_span() {
    // A `[` that is not a class start (no `:` after it) must not let the
    // span walker jump to a later `:]`: the bracket closes at the first
    // unquoted `]`, and the rest is trailing literal pattern.
    assert!(!m(b"[[x:]]", b"x"));
    assert!(m(b"[[x:]]", b"x]"));
    // Same for a `[` whose inner `:` is a member, not a class opener.
    assert!(!m(b"[a[b:]]", b"a"));
    assert!(m(b"[a[b:]]", b"a]"));
    // A non-class `[` keeps its bytes literal members (the `[:name:]`-shaped
    // tail is not a class): `:` stays a member.
    assert!(m(b"[x[.alpha:]]", b":]"));
}

#[test]
fn an_empty_name_class_is_literal_members() {
    // `[:]` is not a recognized class (empty name): its bytes are literal
    // members, and the inner `]` closes the bracket (the last `]` is a
    // trailing literal).
    assert!(m(b"[[:]]", b":]"));
    assert!(m(b"[[:]]", b"[]"));
    assert!(!m(b"[[:]]", b"]"));
    assert!(!m(b"[[:]]", b"x"));
}

#[test]
fn posix_class_quoted_bracket_is_literal() {
    // A quoted `[` does not open a class: the whole word is one literal.
    let mask = [
        Quoted, Unquoted, Unquoted, Unquoted, Unquoted, Unquoted, Unquoted, Unquoted, Unquoted,
    ];
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
    // An unclosed `[:` is not a class: the span is unclosed, no pattern.
    assert!(!super::has_unquoted_pattern(b"x[:b", &[]));
    assert!(!super::has_unquoted_pattern(b"a\\*", &[]));
    assert!(!super::has_unquoted_pattern(b"\\[", &[]));
    assert!(!super::has_unquoted_pattern(b"a*", &[Unquoted, Quoted]));
    assert!(super::has_unquoted_pattern(
        b"a*b",
        &[Unquoted, Unquoted, Unquoted]
    ));
    assert!(!super::has_unquoted_pattern(
        b"a*b",
        &[Unquoted, Quoted, Unquoted]
    ));
    // A leading `^` is a marker, so `[^a]` is a valid bracket; `[^]` is not
    // (`]` is its first member, so the expression is unclosed and the `[`
    // is a literal byte). A quoted marker leaves a valid bracket too.
    assert!(super::has_unquoted_pattern(b"[^a]", &[]));
    assert!(!super::has_unquoted_pattern(b"[^]", &[]));
    assert!(super::has_unquoted_pattern(
        b"[^a]",
        &[Unquoted, Quoted, Unquoted, Unquoted]
    ));
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

/// POSIX #4.1: an escaped pair is a literal byte, so `f\oo\.*` substitutes to
/// `foo.*` with the escaped bytes mask-protected: the escaped `.` is literal
/// and the trailing `*` is the only pattern byte.
#[test]
fn escaped_pairs_are_literal_and_the_rest_globs() {
    // The substituted word: the `.` pair is folded to `.` and mask-protected,
    // so the star globs the tail only after a literal `.`.
    let mask = [Unquoted, Quoted, Unquoted, Quoted, Unquoted];
    assert!(m_masked(b"foo.*", &mask, b"foo.bar"));
    assert!(!m_masked(b"foo.*", &mask, b"foo2"));
    // The token text keeps the pairs: the escaped `.` is literal, the star
    // after it is a pattern byte.
    assert!(m(b"f\\oo\\.*", b"foo.bar"));
    assert!(!m(b"f\\o\\.*", b"fooXbar"));
}

#[test]
fn masked_star_is_not_a_pattern_byte() {
    assert!(!super::has_unquoted_pattern(b"a*", &[Unquoted, Quoted]));
    assert!(super::has_unquoted_pattern(b"a*", &[Unquoted, Unquoted]));
    // The token text keeps the pair, and the `\` skip makes the escaped byte
    // literal: `a\*` has no pattern byte, `a\\*` has a literal `\` pair
    // followed by a pattern `*`.
    assert!(!super::has_unquoted_pattern(b"a\\*", &[]));
    assert!(super::has_unquoted_pattern(b"a\\\\*", &[]));
}
