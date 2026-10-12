#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use alloc::vec::Vec;

/// `${ … }` at the start of `line`: `(body, consumed from the `$`, closed)`.
fn brace_scan(line: &[u8]) -> (Vec<u8>, usize, bool) {
    scan_brace(line, 0)
}

/// `` ` … ` `` at the start of `line`: `(body, consumed from the backtick, closed)`.
fn backtick_scan(line: &[u8]) -> (Vec<u8>, usize, bool) {
    scan_backtick(line, 0)
}

// The body runs to the `}` that closes the brace *and* paren depth; quotes are
// dropped from it, and `consumed` counts every raw byte from the `$` on.
#[test]
fn brace_body_drops_quotes_and_counts_raw_bytes() {
    assert_eq!(
        scan_brace(b"x${y:-\"a}b\"}rest", 1),
        (b"y:-a}b".to_vec(), 11, true),
        "the quoted `}}` is data: 11 bytes from the `$`"
    );
}

// A quote opens and closes inside the body: the inner `}` is quoted data.
#[test]
fn brace_close_inside_quotes_is_data() {
    assert_eq!(
        brace_scan(b"${a\"b}c\"d}"),
        (b"ab}cd".to_vec(), 10, true),
        "the dropped quotes are not body bytes, but they are counted"
    );
}

// Nested `${ … }`: only the outer `}` closes the region.
#[test]
fn nested_brace_region_closes_at_the_outer_brace() {
    assert_eq!(
        brace_scan(b"${x:-${y}}}"),
        (b"x:-${y}".to_vec(), 10, true),
        "the inner close is a body byte"
    );
}

// A `}` inside a `$( … )` region is data: the parens share the depth counter.
#[test]
fn brace_close_inside_a_nested_paren_region_is_data() {
    assert_eq!(
        brace_scan(b"${x:-$(a}b)}"),
        (b"x:-$(a}b)".to_vec(), 12, true),
        "the `}}` at depth 1 is data"
    );
}

// A `)` inside quotes is data, exactly like `paren_scan`'s rule.
#[test]
fn paren_close_inside_quotes_is_data_for_the_brace_scan() {
    assert_eq!(brace_scan(b"${a\"(x)\"}"), (b"a(x)".to_vec(), 9, true));
}

// The escape pair shields the next byte and stays in the body.
#[test]
fn brace_body_escape_pairs_never_count() {
    assert_eq!(
        brace_scan(b"${a\\}b\\)c\\\"d}"),
        (b"a\\}b\\)c\\\"d".to_vec(), 13, true),
        "the escaped quote never toggles the quote state"
    );
}

// An unbalanced `{` widens the region to the matching `}`.
#[test]
fn brace_body_unbalanced_opener_needs_its_close() {
    assert_eq!(brace_scan(b"${a{b}c}"), (b"a{b}c".to_vec(), 8, true));
}

// Unterminated: the whole rest of the line is consumed, `closed == false`.
#[test]
fn brace_body_unterminated_consumes_the_rest_of_the_line() {
    assert_eq!(brace_scan(b"${x"), (b"x".to_vec(), 3, false));
    assert_eq!(brace_scan(b"${x:-y"), (b"x:-y".to_vec(), 6, false));
}

// An unpaired trailing `"` belongs to the enclosing region: the scan stops
// before it and drops the bytes it had taken after it.
#[test]
fn brace_body_unpaired_trailing_quote_is_left_to_the_enclosing_region() {
    assert_eq!(
        brace_scan(b"${v#a\""),
        (b"v#a".to_vec(), 5, false),
        "consumed stops at the quote, which is given back"
    );
    // Same shape with a prefix: `consumed` counts from the `$` at index 2.
    assert_eq!(
        scan_brace(b"x\"${v#a\"", 2),
        (b"v#a".to_vec(), 5, false),
        "the quote at index 7 is 5 bytes from the `$`"
    );
}

// A trailing `\` has no second byte: the pair is not formed, so the backslash
// is counted as consumed but is not part of the body.
#[test]
fn brace_body_trailing_backslash_ends_the_scan() {
    assert_eq!(brace_scan(b"${x}\\"), (b"x".to_vec(), 4, true));
    assert_eq!(
        brace_scan(b"${x\\"),
        (b"x".to_vec(), 4, false),
        "the dangling backslash is consumed, not kept"
    );
}

// The slice form returns the index just past the closing `}`.
#[test]
fn brace_at_returns_the_index_past_the_close() {
    assert_eq!(scan_brace_at(b"${u:-\"z w\"}", 0), Some(11));
    assert_eq!(scan_brace_at(b"${}", 0), Some(3), "the empty body closes");
    assert_eq!(scan_brace_at(b"x${y}", 1), Some(5));
    assert_eq!(scan_brace_at(b"${x", 0), None, "unterminated");
    assert_eq!(
        scan_brace_at(b"${v#a\"", 0),
        None,
        "the unpaired trailing quote is not a close"
    );
    assert_eq!(scan_brace_at(b"abc", 5), None, "start past the line");
}

// `` ` … ` ``: no nesting, the first unescaped backtick closes.
#[test]
fn backtick_body_drops_quotes_and_closes_at_the_backtick() {
    assert_eq!(
        scan_backtick(b"x`printf \"q r\"`tail", 1),
        (b"printf q r".to_vec(), 14, true),
        "the 14th byte from the backtick is the close; the tail stays"
    );
}

// `` \` `` is an escape pair: it never closes the region, and stays in the body.
#[test]
fn backtick_body_escaped_backtick_never_closes() {
    assert_eq!(
        backtick_scan(b"`a\\`b`"),
        (b"a\\`b".to_vec(), 6, true),
        "the 6th byte from the opening backtick closes"
    );
}

// A backtick inside double quotes is data (the quote toggles, not the close).
#[test]
fn backtick_close_inside_quotes_is_data() {
    assert_eq!(
        scan_backtick(b"x`a\"b`c\"", 1),
        (b"ab`c".to_vec(), 7, false),
        "the quotes pair inside the region, so it never closes"
    );
    // With the quotes paired inside, the next backtick closes the region.
    assert_eq!(
        scan_backtick(b"`a\"b`c\"d`", 0),
        (b"ab`cd".to_vec(), 9, true),
        "the backtick after the closing quote closes it"
    );
}

// An unpaired trailing `"` is given back to the enclosing region.
#[test]
fn backtick_body_unpaired_trailing_quote_is_left_to_the_enclosing_region() {
    assert_eq!(
        backtick_scan(b"`a\"b"),
        (b"a".to_vec(), 2, false),
        "the backtick and `a` consumed, the quote given back"
    );
}

// Unterminated (no quote open at the end): the whole rest is consumed.
#[test]
fn backtick_body_unterminated_consumes_the_rest_of_the_line() {
    assert_eq!(
        backtick_scan(b"`printf q r"),
        (b"printf q r".to_vec(), 11, false)
    );
    assert_eq!(
        backtick_scan(b"`a\\"),
        (b"a".to_vec(), 3, false),
        "the dangling backslash is consumed, not kept"
    );
}

// The backtick slice form returns the index just past the closing backtick.
#[test]
fn backtick_at_returns_the_index_past_the_close() {
    assert_eq!(scan_backtick_at(b"`printf \"q r\"`", 0), Some(14));
    assert_eq!(scan_backtick_at(b"x`a`", 1), Some(4));
    assert_eq!(
        scan_backtick_at(b"`a\\`", 0),
        None,
        "the only close is escaped"
    );
    assert_eq!(scan_backtick_at(b"`a", 0), None, "unterminated");
    assert_eq!(scan_backtick_at(b"abc", 5), None, "start past the line");
}

/// A `'…'` span is one opaque literal (task #141): the `}` written in it is
/// data, its quote bytes are dropped from the body like a `"`'s, and the
/// region closes at the real `}`.
#[test]
fn brace_span_is_opaque_and_its_quote_bytes_are_dropped() {
    assert_eq!(
        brace_scan(b"${a:-'b}c'}"),
        (b"a:-b}c".to_vec(), 11, true),
        "the closing brace written inside the span is data"
    );
    // A backslash written in the span is literal: it shields no byte, so the
    // `}` written behind it is span data and the real `}` still closes.
    assert_eq!(
        brace_scan(b"${a:-'b\\}'}"),
        (b"a:-b\\}".to_vec(), 11, true),
        "the span's backslash shields nothing; the span's brace is data"
    );
}

/// A backtick body containing a `'…'` span ends at the real closing backtick,
/// not at the one written inside the span.
#[test]
fn backtick_span_content_is_data() {
    assert_eq!(
        backtick_scan(b"`echo 'a`b'`"),
        (b"echo a`b".to_vec(), 12, true),
        "the span's backtick is data; its quote bytes are dropped like a `\"`'s"
    );
    // A `'` left open at the end of the line belongs to the enclosing region.
    let (body, consumed, closed) = backtick_scan(b"`echo 'a");
    assert!(!closed);
    assert_eq!(
        (body, consumed),
        (b"echo ".to_vec(), 6),
        "the scan stops before the unpaired span quote"
    );
}

/// An unterminated region gives its opening quote back to the enclosing scan,
/// and the offset of that quote is measured **from the region start** — for a
/// `'…'` span exactly as for a `"` (the region model drops the quote bytes; see
/// the README's Quoting divergences). The pins use a nonzero start so the
/// offset is measured, not the absolute index.
#[test]
fn unterminated_span_offset_counts_from_the_region_start() {
    // `echo ${v:-'a b`: `$` at 5, the span's `'` at 10 → 5 raw bytes are the
    // region (`${` + `v:-`), and the body is truncated at the span quote.
    assert_eq!(
        scan_brace(b"echo ${v:-'a b", 5),
        (b"v:-".to_vec(), 5, false),
        "the span quote is at offset 5 from the `$`"
    );
    // ``x`echo 'a``: the backtick is at 1, the span's `'` at 7 → 6 bytes.
    assert_eq!(
        scan_backtick(b"x`echo 'a", 1),
        (b"echo ".to_vec(), 6, false),
        "the span quote is at offset 6 from the backtick"
    );
    // The `"` arm keeps the same rule at a nonzero start (regression pair).
    assert_eq!(
        scan_brace(b"echo ${v:-\"a b", 5),
        (b"v:-".to_vec(), 5, false)
    );
}
