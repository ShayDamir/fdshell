#![allow(clippy::expect_used)]

use alloc::vec;
use alloc::vec::Vec;

use super::{fold, fold_mask, fold_quotes, fold_word};
use sys::ShortCStr;

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

/// A delimiter word is a word: POSIX quote removal drops **every** quote
/// marker byte, not one surrounding pair, and reports a quote byte in any
/// position (task #207).
#[test]
fn fold_quotes_strips_every_quote_byte() {
    assert_eq!(fold_quotes(b"EOF"), (b"EOF".to_vec(), false));
    assert_eq!(fold_quotes(b""), (Vec::new(), false));
    assert_eq!(fold_quotes(b"\"Q\""), (b"Q".to_vec(), true));
    assert_eq!(fold_quotes(b"''"), (Vec::new(), true));
    assert_eq!(fold_quotes(b"\"\""), (Vec::new(), true));
    assert_eq!(fold_quotes(b"a'b'c"), (b"abc".to_vec(), true));
    assert_eq!(fold_quotes(b"'x'y'z'"), (b"xyz".to_vec(), true));
    assert_eq!(fold_quotes(b"ab'c'"), (b"abc".to_vec(), true));
    assert_eq!(fold_quotes(b"'a'bc"), (b"abc".to_vec(), true));
    assert_eq!(fold_quotes(b"E\"O\"F"), (b"EOF".to_vec(), true));
}

/// The spaces of a quoted span are word content, so they survive into the
/// delimiter text (`<<"a b"` and `<<'a <<b'` delimit a word with a space).
#[test]
fn fold_quotes_keeps_the_spaces_of_a_quoted_span() {
    assert_eq!(fold_quotes(b"a\"b c\"d"), (b"ab cd".to_vec(), true));
    assert_eq!(fold_quotes(b"'a <<b'"), (b"a <<b".to_vec(), true));
}

/// The folding runs in the same pass: an unquoted pair folds and the trailing
/// `\` keeps its backslash. A folded pair is a POSIX quote operator, so it
/// **quotes** the word (task #263); the trailing lone `\` is no pair and
/// leaves the word unquoted.
#[test]
fn fold_quotes_folds_unquoted_escape_pairs_in_the_same_pass() {
    assert_eq!(fold_quotes(b"a\\*b"), (b"a*b".to_vec(), true));
    assert_eq!(fold_quotes(b"E\\OF"), (b"EOF".to_vec(), true));
    // A trailing `\` keeps it, exactly like `fold`, and quotes nothing.
    assert_eq!(fold_quotes(b"E\\"), (b"E\\".to_vec(), false));
}

/// An escaped quote is data, not a marker: it stays in the text (folded to the
/// quote byte). The pair that produced it is a quote operator, so the word is
/// quoted — the *text* and the *flag* answer different questions (task #263).
/// Folding first and stripping in the same pass is what makes this decidable
/// (LESSONS).
#[test]
fn fold_quotes_keeps_an_escaped_quote_as_data() {
    assert_eq!(fold_quotes(b"a\\'b"), (b"a'b".to_vec(), true));
    assert_eq!(fold_quotes(b"a\\\"b"), (b"a\"b".to_vec(), true));
}

/// A quoted span keeps its pair: POSIX #4.2 drops the backslash only at
/// substitution, so `<<"E\OF"` delimits the literal `E\OF`.
#[test]
fn fold_quotes_keeps_a_quoted_escape_pair() {
    assert_eq!(fold_quotes(b"\"E\\OF\""), (b"E\\OF".to_vec(), true));
}

/// POSIX XCU 2.3 counts `\` a quote operator, so a word quoted *only* by
/// escape pairs is quoted (`<<\E\O\F`, `<<E\OF`): the body is literal. The two
/// negative rows are the kill tests for "set the flag on any `\` byte" — a
/// trailing lone `\` escapes nothing, so it is no pair and quotes nothing.
#[test]
fn fold_quotes_counts_a_folded_escape_pair_as_a_quote_operator() {
    for (raw, want) in [
        (b"E\\OF" as &[u8], b"EOF" as &[u8]),
        (b"\\E\\O\\F", b"EOF"),
        (b"\\\\EOF", b"\\EOF"),
        (b"\\$HOME", b"$HOME"),
        (b"a\\'b", b"a'b"),
        (b"a\\\"b", b"a\"b"),
        (b"a\\*b", b"a*b"),
    ] {
        let (got, quoted) = fold_quotes(raw);
        assert_eq!(got, want, "raw={raw:?}");
        assert!(quoted, "a folded pair quotes the word: raw={raw:?}");
    }
    // No pair, no quote operator: a trailing `\`, and a plain word.
    assert_eq!(fold_quotes(b"E\\"), (b"E\\".to_vec(), false));
    assert_eq!(fold_quotes(b"EOF"), (b"EOF".to_vec(), false));
}

/// Nesting: a `"` written in a `'…'` span is data, a `'` written in `"…"` is
/// data (README "Quoting"). The quote byte seen is a marker byte, so both
/// words are quoted.
#[test]
fn fold_quotes_treats_a_quote_in_the_other_span_as_data() {
    assert_eq!(fold_quotes(b"'a\"b'"), (b"a\"b".to_vec(), true));
    assert_eq!(fold_quotes(b"\"a'b\""), (b"a'b".to_vec(), true));
}

#[test]
fn mask_fold_folds_unquoted_pair_and_protects_the_escaped_byte() {
    // Word text carries no `"` bytes, so the quote state is the mask.
    assert_eq!(
        fold_mask(b"a\\*b", &[false, false, false, false]),
        (b"a*b".to_vec(), vec![false, true, false])
    );
    // `\\` folds to one backslash, protected from splitting and globbing.
    assert_eq!(
        fold_mask(b"a\\\\b", &[false, false, false, false]),
        (b"a\\b".to_vec(), vec![false, true, false])
    );
}

#[test]
fn mask_fold_keeps_a_quoted_pair() {
    // POSIX #4.2: every byte of the pair was inside double quotes.
    assert_eq!(
        fold_mask(b"a\\*b", &[true, true, true, true]),
        (b"a\\*b".to_vec(), vec![true, true, true, true])
    );
}

#[test]
fn mask_fold_keeps_a_trailing_backslash() {
    assert_eq!(
        fold_mask(b"a\\", &[false, false]),
        (b"a\\".to_vec(), vec![false, false])
    );
    assert_eq!(fold_mask(b"", &[false]), (Vec::new(), Vec::new()));
    // A short mask reads as unquoted.
    assert_eq!(fold_mask(b"a\\b", &[]), (b"ab".to_vec(), vec![false, true]));
}

#[test]
fn mask_fold_mixes_unquoted_and_quoted_regions() {
    // An unquoted pair followed by quoted bytes: the index bookkeeping must
    // keep reading the mask at the right byte.
    assert_eq!(
        fold_mask(b"a\\*bc", &[false, false, false, false, true]),
        (b"a*bc".to_vec(), vec![false, true, false, true])
    );
    // Quoted bytes first, then an unquoted pair.
    assert_eq!(
        fold_mask(b"ab\\*c", &[true, true, false, false, false]),
        (b"ab*c".to_vec(), vec![true, true, true, false])
    );
    // A quoted pair in the middle keeps its backslash and the rest folds.
    assert_eq!(
        fold_mask(b"a\\b\\*c", &[false, true, true, false, false]),
        (b"a\\b*c".to_vec(), vec![false, true, true, true, false])
    );
}

#[test]
fn fold_word_returns_a_shortcstr_with_the_aligned_mask() {
    let (word, mask) = fold_word(&ShortCStr::from(c"a\\*b"), &[false, false, false, false])
        .expect("folded word is NUL-free");
    assert_eq!(word.as_bytes().expect("bytes"), b"a*b");
    assert_eq!(mask, vec![false, true, false]);
    // A word with no pair is returned unchanged (a fresh ShortCStr).
    let (word, mask) = fold_word(&ShortCStr::from(c"echo"), &[false, false, false, false])
        .expect("folded word is NUL-free");
    assert_eq!(word.as_bytes().expect("bytes"), b"echo");
    assert_eq!(mask, vec![false, false, false, false]);
}

/// A `'…'` span has no escape pair at all (POSIX §2.2.3): every byte written in
/// it is literal, including a backslash. Its state is independent from the `"`
/// state — `fold` reads the raw bytes, so a delimiter text still carries quotes.
#[test]
fn fold_keeps_every_byte_of_a_single_quote_span() {
    // The span's backslash is one byte of data: no folding happens in the span.
    assert_eq!(fold(b"'a\\*b'"), b"'a\\*b'");
    assert_eq!(fold(b"'a\\'"), b"'a\\'");
    // An unquoted pair behind the span folds as usual.
    assert_eq!(fold(b"'a\\b'\\*c"), b"'a\\b'*c");
    // A `"` written in a span is data: it opens no double-quote state, so the
    // pair behind the span is unquoted and folds.
    assert_eq!(fold(b"'a\"b'\\*c"), b"'a\"b'*c");
    // A `'` written inside double quotes is data: the pair stays quoted.
    assert_eq!(fold(b"\"a'b\\*c\""), b"\"a'b\\*c\"");
}

/// A span's literal bytes arrive from the tokenizer in the double-quote escape
/// form (`\\`, `\$`) with mask `true`, and a word that never reaches
/// substitution is folded by this function alone: POSIX #4.2 drops the
/// backslash of a quoted `\$`/`\\` pair, so the span round-trips to the bytes
/// that were written in it. Any other quoted pair keeps its backslash.
#[test]
fn fold_mask_round_trips_the_single_quote_encoding() {
    // `'a\b'` → word `a\\b`, mask all quoted → `a\b`.
    assert_eq!(
        fold_mask(b"a\\\\b", &[true, true, true, true]),
        (b"a\\b".to_vec(), vec![true, true, true])
    );
    // `'$'` → word `\$`, mask quoted → `$` (no substitution pass involved).
    assert_eq!(
        fold_mask(b"\\$", &[true, true]),
        (b"$".to_vec(), vec![true])
    );
    // An unquoted pair folds the same way, protecting the escaped byte.
    assert_eq!(
        fold_mask(b"\\$", &[false, false]),
        (b"$".to_vec(), vec![true])
    );
    // Any other quoted pair keeps its backslash (POSIX #4.2 lists only these).
    assert_eq!(
        fold_mask(b"a\\*b", &[true, true, true, true]),
        (b"a\\*b".to_vec(), vec![true, true, true, true])
    );
}
