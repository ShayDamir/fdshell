use super::fold;

#[test]
fn folds_unquoted_escape_pairs() {
    // POSIX #4.1: the backslash is removed and the escaped byte loses meaning.
    assert_eq!(fold(b"a\\*b"), b"a*b");
    assert_eq!(fold(b"a\\ b"), b"a b");
    // A `\\` pair folds to a single backslash.
    assert_eq!(fold(b"a\\\\b"), b"a\\b");
}

#[test]
fn keeps_backslash_inside_double_quotes() {
    // POSIX #4.2: inside a quoted span the pair is kept for the builtin.
    assert_eq!(fold(b"\"a\\*b\""), b"\"a\\*b\"");
    // The quote state resets per slice, so a later unquoted pair folds.
    assert_eq!(fold(b"\"a\"\\*b"), b"\"a\"*b");
}

#[test]
fn keeps_trailing_backslash() {
    assert_eq!(fold(b"a\\"), b"a\\");
    assert_eq!(fold(b"\\\\"), b"\\");
}

#[test]
fn plain_bytes_are_unchanged() {
    assert_eq!(fold(b"EOF"), b"EOF");
    assert_eq!(fold(b""), b"");
}
