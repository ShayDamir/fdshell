#![allow(clippy::unwrap_used)]

/// Brace-scan a `name() { … }` line and return its exclusive end + closed flag.
fn scan(line: &[u8]) -> Option<(usize, bool)> {
    crate::brace::scan_function_block(line, line, 0, false)
}

#[test]
fn closes_at_matching_brace() {
    let line = b"f() { x=1; }";
    assert_eq!(scan(line), Some((line.len(), true)));
}

#[test]
fn nested_brace_in_param_expansion_balances() {
    // `${a:-b}` adds a balanced inner brace pair; the final `}` still closes.
    let line = b"f() { x=${a:-b}; }";
    assert_eq!(scan(line), Some((line.len(), true)));
}

#[test]
fn quoted_braces_are_not_counted() {
    let close = b"f() { v=\"}\"; }";
    assert_eq!(scan(close), Some((close.len(), true)));
    let open = b"f() { v=\"{\"; }";
    assert_eq!(scan(open), Some((open.len(), true)));
}

#[test]
fn brace_inside_command_subst_is_not_counted() {
    let line = b"f() { v=$(echo {); }";
    assert_eq!(scan(line), Some((line.len(), true)));
}

#[test]
fn brace_inside_backticks_is_not_counted() {
    let line = b"f() { v=`echo {`; }";
    assert_eq!(scan(line), Some((line.len(), true)));
}

#[test]
fn comment_runs_to_end_and_leaves_block_unclosed() {
    // A `#` comment runs to end of line, so a `}` after it is unreachable.
    let r = scan(b"f() { x=1 # c");
    assert!(r.is_some_and(|(_, closed)| !closed));
}

#[test]
fn non_function_is_rejected() {
    assert_eq!(scan(b"plain command"), None);
}

// A `}`-shaped heredoc body line must not close the block; only the real
// `}` does.
#[test]
fn heredoc_body_brace_does_not_close() {
    let line = b"f() { cat <<EOF\n}\nEOF\n}";
    assert_eq!(scan(line), Some((line.len(), true)));
}

// A nested `{`-shaped body line must not open a brace depth level.
#[test]
fn heredoc_body_open_brace_does_not_open_depth() {
    let line = b"f() { cat <<EOF\n{\nEOF\n}";
    assert_eq!(scan(line), Some((line.len(), true)));
}
