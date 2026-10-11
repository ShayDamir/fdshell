use super::*;

// The `(` arm with a *short* region: its `)` sits before `2 * i`, so the index
// arithmetic that starts the scan just past `$(` is pinned — `i + 2` finds the
// close, any `i * 2`-shaped jump lands past it and reports no region (LESSONS:
// mutation coverage, STYLE.md §5l).
#[test]
fn short_dollar_paren_region_resumes_just_past_its_close() {
    // `echo "$(a)"`: the region is 6..=9, so scanning continues at 10.
    assert_eq!(skip_quoted_region(b"echo \"$(a)\"", 6), Some(10));
    assert_eq!(skip_quoted_region(b"$(a)", 0), Some(4));
    assert_eq!(
        skip_quoted_region(b"$()", 0),
        Some(3),
        "the empty body closes"
    );
}

// The `(` arm with a full-length region: inner quotes and separators are
// inside it, so the resume index is the one just past the `)` that closes the
// region's own paren depth.
#[test]
fn dollar_paren_region_is_skipped_whole() {
    assert_eq!(
        skip_quoted_region(b"echo \"$(echo \"a; b\")\"", 6),
        Some(20),
        "the quoted `;` is region data"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"$(printf \"x && y\")\"", 6),
        Some(24),
        "the quoted `&&` is region data"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"$(echo \"$(echo deep)\")\"", 6),
        Some(28),
        "the inner `$( … )` closes first"
    );
}

// Nested parens share the region's depth counter, and a trailing backslash has
// no pair: the scan stops instead of running off the slice.
#[test]
fn dollar_paren_region_counts_nested_parens() {
    assert_eq!(
        skip_quoted_region(b"echo \"$((1+2))\"", 6),
        Some(14),
        "the inner pair must be consumed before the close counts"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"$(a(b)c)\"", 6),
        Some(14),
        "the close at 13 belongs to the region"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"$(a\\", 6),
        None,
        "a dangling backslash ends the scan"
    );
}

// The `${ … }` arm: the resume index is just past the `}` that closes the
// brace depth, with quoted `}` and nested `${ … }` treated as data.
#[test]
fn brace_region_is_skipped_whole() {
    assert_eq!(
        skip_quoted_region(b"echo \"${u:-\"a b\"}\"", 6),
        Some(17),
        "the closing brace at 16 ends the region"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"${x:-${y}}\"", 6),
        Some(16),
        "only the outer brace closes the region"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"${u:-\"a}b\"}\"", 6),
        Some(17),
        "the quoted `}}` is region data"
    );
    assert_eq!(skip_quoted_region(b"echo \"${}\"", 6), Some(9));
}

// The backtick arm: no nesting, the first unescaped, unquoted backtick closes
// the region.
#[test]
fn backtick_region_is_skipped_whole() {
    assert_eq!(
        skip_quoted_region(b"echo \"`printf \"q r\"`\"", 6),
        Some(20),
        "the closing backtick at 19 ends the region"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"`printf \"q;\"`\"", 6),
        Some(19),
        "the quoted `;` is region data"
    );
    assert_eq!(skip_quoted_region(b"echo \"`a`\"", 6), Some(9));
    assert_eq!(
        skip_quoted_region(b"echo \"`a\\`b`\"", 6),
        Some(12),
        "the escaped backtick never closes the region"
    );
}

// A byte that opens no region, a `$` whose next byte is not `(` or `{`, and a
// `$` at the end of the line are all `None`: the caller keeps scanning byte
// by byte, which is the pre-#130 behavior the unbalanced cases depend on.
#[test]
fn bytes_that_open_no_region_are_none() {
    assert_eq!(skip_quoted_region(b"echo \"a\"", 6), None, "a word byte");
    assert_eq!(skip_quoted_region(b"echo \"$x\"", 6), None, "a bare `$`");
    assert_eq!(
        skip_quoted_region(b"echo \"$\"", 6),
        None,
        "`$` before the quote"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"$\"", 7),
        None,
        "the closing quote opens nothing"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"$\"", 8),
        None,
        "index past the line"
    );
}

// An unbalanced region is `None`, including the one whose only close is quoted
// (the region never closes) and the trailing `$` with nothing after it.
#[test]
fn unclosed_regions_are_none() {
    assert_eq!(
        skip_quoted_region(b"echo \"$(\"", 6),
        None,
        "unclosed `$( … )`"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"$(x", 6),
        None,
        "no close in the tail"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"$(a\")\"", 6),
        None,
        "the only `)` is quoted data"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"${x", 6),
        None,
        "unclosed `${{ … }}`"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"${v#a\"", 6),
        None,
        "the unpaired trailing quote is not a close"
    );
    assert_eq!(
        skip_quoted_region(b"echo \"`x", 6),
        None,
        "unclosed backtick"
    );
    assert_eq!(skip_quoted_region(b"echo \"$", 6), None, "trailing `$`");
}
