#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::tokenize;
use alloc::{vec, vec::Vec};
use sys::ShortCStr;

/// The word text plus its quote mask of every token of `line`.
fn words(line: &[u8]) -> Vec<(ShortCStr, Vec<bool>)> {
    tokenize(line)
        .unwrap()
        .into_iter()
        .map(|(t, _, _, _, mask)| (t, mask))
        .collect()
}

/// The word text of every token of `line`.
fn text(line: &[u8]) -> Vec<ShortCStr> {
    words(line).into_iter().map(|(t, _)| t).collect()
}

// POSIX #4.1: the escape pair stays in the token text, so the byte offsets and
// the token-text syntax prefixes (`\if`, `\)`, `\<<`) keep working.
#[test]
fn escape_pair_stays_in_the_word_text() {
    let w = words(b"echo a\\*");
    assert_eq!(w[1].0, ShortCStr::from(c"a\\*"));
    assert_eq!(
        w[1].1,
        vec![false, false, false],
        "the pair bytes are unquoted"
    );
}

// A trailing `\` at end of line is a lone word byte (bash prints a literal `\`).
#[test]
fn trailing_backslash_at_eof_is_kept() {
    assert_eq!(
        text(b"echo a\\"),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"a\\")]
    );
}

// An escaped `;` is a word byte: one token, no separator token.
#[test]
fn escaped_semicolon_is_one_word() {
    assert_eq!(
        text(b"echo a\\;b"),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"a\\;b")]
    );
}

// An escaped `|` never splits a pipeline.
#[test]
fn escaped_pipe_is_one_word() {
    assert_eq!(
        text(b"echo a\\|b"),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"a\\|b")]
    );
}

// An escaped `)` does not emit the separator token the bare `)` arm pushes.
#[test]
fn escaped_paren_emits_no_separator() {
    assert_eq!(
        text(b"echo a\\)b"),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"a\\)b")]
    );
}

// `\<newline>` is a word byte pair (the line continuation), not a line end.
#[test]
fn escaped_newline_stays_in_the_word() {
    assert_eq!(
        text(b"echo x\\\ny"),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"x\\\ny")]
    );
}

// An escaped `#` is a word byte, so it never starts a comment.
#[test]
fn escaped_hash_is_a_word_byte() {
    assert_eq!(
        text(b"echo \\#hi"),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"\\#hi")]
    );
}

// A leading escape pair makes the word un-prefixable: `\if` is one word, not a
// keyword, and `a\>b` is one word, not a redirect.
#[test]
fn escape_pair_breaks_token_text_prefixes() {
    assert_eq!(
        text(b"\\if a"),
        [ShortCStr::from(c"\\if"), ShortCStr::from(c"a")]
    );
    assert_eq!(
        text(b"echo a\\>b"),
        [ShortCStr::from(c"echo"), ShortCStr::from(c"a\\>b")]
    );
}

// Inside double quotes the POSIX #4.2 rules are unchanged: `\$`/`\\` keep both
// bytes for substitution, `\"` yields a literal `"`, any other pair keeps the
// backslash, and every byte is mask-protected.
#[test]
fn quoted_escape_rules_are_unchanged() {
    let w = words(b"echo \"a\\\\b\" \"a\\$X\" \"a\\\"b\" \"a\\nc\"");
    assert_eq!(w[1].0, ShortCStr::from(c"a\\\\b"));
    assert_eq!(w[2].0, ShortCStr::from(c"a\\$X"));
    assert_eq!(w[3].0, ShortCStr::from(c"a\"b"));
    assert_eq!(w[4].0, ShortCStr::from(c"a\\nc"));
    assert!(
        w[1].1.iter().all(|q| *q),
        "quoted bytes stay mask-protected"
    );
}

// POSIX #2.2 `>|`: a `|` right after a `>` operator byte is absorbed into the
// operator word, so `>|file`, `2>|file` and the bare `>|` are one token each.
#[test]
fn clobber_pipe_absorbed_into_operator() {
    assert_eq!(
        text(b"echo a >|b"),
        [
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"a"),
            ShortCStr::from(c">|b")
        ]
    );
    assert_eq!(
        text(b"echo a >| b"),
        [
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"a"),
            ShortCStr::from(c">|"),
            ShortCStr::from(c"b"),
        ]
    );
    assert_eq!(
        text(b"echo hi 2>|f"),
        [
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"hi"),
            ShortCStr::from(c"2>|f"),
        ]
    );
}

// A `|` separated by whitespace from a bare `>` is a pipeline pipe: only a `|`
// whose word ends *at* the `>` byte is absorbed (`> | f` stays three tokens).
#[test]
fn spaced_pipe_after_bare_gt_stays_a_pipe() {
    assert_eq!(
        text(b"echo a > | b"),
        [
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"a"),
            ShortCStr::from(c">"),
            ShortCStr::from(c"|"),
            ShortCStr::from(c"b"),
        ]
    );
}

// `&>|f` is absorbed as one token, so `&>|` reaches `parse_bg_redirect` as a
// `|` tail and is rejected there; the `&>|&pid` pidvar form stays one token.
#[test]
fn amp_clobber_is_one_token() {
    assert_eq!(
        text(b"cmd &>|f"),
        [ShortCStr::from(c"cmd"), ShortCStr::from(c"&>|f")]
    );
    assert_eq!(
        text(b"cmd &>|&pid"),
        [ShortCStr::from(c"cmd"), ShortCStr::from(c"&>|&pid")]
    );
}

// A word that *carries* the operator keeps the absorbed `|` as a word byte:
// accepted deviation (fdshell never breaks a word at operator bytes, as for
// `a>b`), pinned so the deviation is measured, not assumed.
#[test]
fn prefix_word_absorbs_pipe_is_one_word() {
    assert_eq!(
        text(b"echo x a>|b"),
        [
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"x"),
            ShortCStr::from(c"a>|b")
        ]
    );
}

// The absorbed `|` byte is unquoted (mask `false`), so the target's mask stays
// `true` over exactly the quoted path bytes.
#[test]
fn clobber_absorbed_pipe_is_unquoted_in_the_mask() {
    let w = words(b"echo hi >|\"my file\"");
    assert_eq!(w[2].0, ShortCStr::from(c">|my file"));
    assert!(!w[2].1[0], "the `>` byte is unquoted");
    assert!(!w[2].1[1], "the absorbed `|` byte is unquoted");
    assert_eq!(
        w[2].1[2..],
        vec![true; 7],
        "the quoted path is mask-protected"
    );
}
