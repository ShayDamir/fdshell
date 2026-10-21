#![allow(clippy::unwrap_used)]
use super::*;
use alloc::vec::Vec;

/// The definer name `function_def_name` reports, as bytes.
fn def(word: &[u8]) -> Option<Vec<u8>> {
    super::function_def_name(word, word, 0).map(|w| w.to_vec())
}

#[test]
fn name_len_of_definer_words() {
    assert_eq!(name_len(b"f("), Some(1));
    assert_eq!(name_len(b"ab("), Some(2));
    assert_eq!(name_len(b"f"), Some(1));
    assert_eq!(name_len(b"-a"), Some(2));
    assert_eq!(name_len(b"_a1"), Some(3));
}

#[test]
fn name_len_rejects_non_definers() {
    // A `(` must be the last byte: an inner `(` is a `name(arg)` form.
    assert_eq!(name_len(b"f(x"), None);
    assert_eq!(name_len(b"("), None);
    // A brace in the word is not a name, and a word needs a byte.
    assert_eq!(name_len(b"f{"), None);
    assert_eq!(name_len(b"}"), None);
    assert_eq!(name_len(b""), None);
    // `)` is a word terminator, so it never belongs to a definer word
    // (a quoted name keeps one, e.g. the token `)b` of `function ")b" {`).
    assert_eq!(name_len(b"a)b"), None);
    assert_eq!(name_len(b")"), None);
    // Whitespace splits tokens, so a definer word never holds it.
    assert_eq!(name_len(b"a b"), None);
    assert_eq!(name_len(b"a\tb"), None);
}

/// The four accepted keyword shapes, plus the POSIX form.
#[test]
fn definer_words_of_the_accepted_forms() {
    assert_eq!(def(b"f() { echo hi"), Some(b"f".to_vec()));
    assert_eq!(def(b"function f { echo hi"), Some(b"f".to_vec()));
    assert_eq!(def(b"function f() {"), Some(b"f".to_vec()));
    assert_eq!(def(b"function f () {"), Some(b"f".to_vec()));
    assert_eq!(def(b"function f ( ) {"), Some(b"f".to_vec()));
    // Extra whitespace between the name and the brace is skipped.
    assert_eq!(def(b"function f  {"), Some(b"f".to_vec()));
    // A wide gap is walked, not summed: the gate lands on the name word itself.
    assert_eq!(def(b"function  f {"), Some(b"f".to_vec()));
    assert_eq!(def(b"function\tf {"), Some(b"f".to_vec()));
    // A reserved word as the name (the POSIX `if() {…}` leniency).
    assert_eq!(def(b"function if {"), Some(b"if".to_vec()));
    // An option-shaped name is a name.
    assert_eq!(def(b"function -a f {"), Some(b"-a".to_vec()));
}

/// The quote gate (task #247): a definer word written with quote bytes is
/// word data, exactly as the token-level gate in `parse::function_block`
/// reads it — bash rejects all four spellings (`'f'`: not a valid identifier
/// for the name, a syntax error for the quoted keyword), and the byte-level
/// scan must not claim the block either, or its quote-removal rewrites the
/// definer into a shape the parse layer accepts.
#[test]
fn quoted_definer_words_are_not_definers() {
    assert_eq!(def(b"function \"f\" {"), None);
    assert_eq!(def(b"function 'f'() {"), None);
    assert_eq!(
        def(b"f'oo'() {"),
        None,
        "the POSIX name word carries quotes"
    );
    assert_eq!(
        def(b"foo'('() {"),
        None,
        "a quote glued behind the name is a quote in the word"
    );
    // The gate reads the line at `start`, so a quoted keyword behind a
    // leading offset is caught at the offset, not at 0.
    assert_eq!(
        super::function_def_name(b"\'function\' f (", b"  'function' f (", 2),
        None
    );
    assert_eq!(
        super::function_def_name(b"f() {", b"  f() {", 2),
        Some(b"f".as_slice())
    );
    // The gap is walked on `line`, so a wide gap makes each offset of the walk
    // measurable: `function` + gap 2 + a quoted name of length 2/3, and a tab
    // gap. A sum of those offsets lands the gate outside the name word.
    assert_eq!(def(b"function  '' {"), None);
    assert_eq!(def(b"function  'f' {"), None);
    assert_eq!(def(b"function\t'f' {"), None);
    // A name whose *last* byte is the quote byte, at a gap of 2: the gate must
    // land on the name word's own end, since a one-byte shift drops that byte
    // and the word reads as unquoted.
    assert_eq!(def(b"function  f\" {"), None);
    // The same, at a `start` that is not the line's beginning.
    assert_eq!(
        super::function_def_name(b"function  'f' {", b"   function  'f' {", 3),
        None
    );
    assert_eq!(
        super::function_def_name(b"function  f {", b"   function  f {", 3),
        Some(b"f".as_slice())
    );
    assert_eq!(
        super::function_def_name(b"'f'() {", b"  'f'() {", 2),
        None,
        "the POSIX gate reads the name at the offset, not at 0"
    );
}

#[test]
fn definer_words_of_the_rejected_forms() {
    // No name after `function`, and `function` glued to the opener.
    assert_eq!(def(b"function { echo hi"), None);
    assert_eq!(def(b"function(f) {"), None);
    assert_eq!(def(b"functionf f {"), None);
    assert_eq!(def(b"function"), None);
    // A name glued to `{`, a name with arguments, and a plain word.
    assert_eq!(def(b"function f{ echo hi"), None);
    assert_eq!(def(b"function f(x) {"), None);
    assert_eq!(def(b"foo { x }"), None);
    // The POSIX form needs its `()` opener.
    assert_eq!(def(b"function f}"), None);
    assert_eq!(def(b"() { x }"), None);
}

/// `)` is a word terminator, so a leading `)` never starts a definer word.
#[test]
fn close_paren_is_a_word_terminator() {
    assert_eq!(def(b") f {"), None);
    assert_eq!(def(b"function ) f {"), None);
    // The `)` after a POSIX `name(` ends the definer word.
    assert_eq!(def(b"f() { x }"), Some(b"f".to_vec()));
}

/// A `)` glued to the preceding word is never consumed as a terminator: it is
/// the tokenizer's own separator token, so it is the next word, and the byte
/// split stays aligned with the token split at every position.
#[test]
fn glued_close_paren_after_the_keyword_is_its_own_word() {
    // `function` + `)` + `f`: the name word is `)`, which is not a definer name.
    assert_eq!(def(b"function)f g {"), None);
    assert_eq!(def(b"function)f {"), None);
    // A `)` after the name word ends the definer pair, as in the POSIX form.
    assert_eq!(def(b"function f)g {"), Some(b"f".to_vec()));
}

// Task #235: the definer's byte length — the definer words the parse layer
// consumes; the text after them is `definer_at_run`'s filler test.
#[test]
fn definer_len_spans_the_definer_words() {
    // POSIX: the name word and the `)` closing its `(`.
    assert_eq!(definer_len(b"f(", b"f(", 0), Some(2));
    assert_eq!(definer_len(b"f()", b"f()", 0), Some(3));
    assert_eq!(definer_len(b"f(){", b"f(){", 0), Some(3));
    // Keyword form: `function` + the name word, parens included when glued.
    assert_eq!(definer_len(b"function f", b"function f", 0), Some(10));
    assert_eq!(definer_len(b"function f()", b"function f()", 0), Some(11));
    // A split `()` after the name is not definer words — it is filler for the
    // run rule (`function f ( ) {` is accepted, LESSONS "Byte-level definer
    // words must be the tokenizer's tokens").
    assert_eq!(
        definer_len(b"function f ( )", b"function f ( )", 0),
        Some(10)
    );
    // A word after the definer words is no definer text either: `f` is the
    // name and the trailing `x` run is dropped by the parse layer.
    assert_eq!(definer_len(b"function f x", b"function f x", 0), Some(10));
    // A wide gap does not change the definer words' length.
    assert_eq!(definer_len(b"function  f", b"function  f", 0), Some(11));
}

#[test]
fn definer_len_rejects_non_definers() {
    // POSIX needs its glued `(`; `function` glued to the opener has no name.
    assert_eq!(definer_len(b"f () ", b"f () ", 0), None);
    assert_eq!(definer_len(b"function)f", b"function)f", 0), None);
    assert_eq!(definer_len(b"", b"", 0), None);
}

/// Task #277: the byte word walk consumes a `$( … )` region **whole**, because
/// `parse/token/step.rs` reads it as one word. Ungated, `$( echo true )` at a
/// run start split to the word `$(`, `name_len` read it as the definer of the
/// function `$`, and the run died as an unclosed `name() {` block where bash
/// runs `true`. The same walk owns the assignment-value spelling.
#[test]
fn definer_word_consumes_a_substitution_region() {
    for word in [
        b"$( echo true )".as_slice(),
        b"$( echo true)",
        b"$(  echo true )",
        b"x=$( echo a )",
        b"$(echo a)b(",
        b"$(echo a)(",
        b"a$(b",
        b"x=$( echo a ) f() {",
        // The empty region: its `)` is the word's last byte, `(` is not.
        b"$()",
        // A region behind a name-shaped head is still one word.
        b"f$( echo x )(",
    ] {
        assert_eq!(def(word), None, "a region-carrying word is no definer");
    }
    // An unterminated region claims the rest of the word — and the walk
    // terminates (a `map_or(0, …)`/`map_or(i, …)` jump hangs the scan).
    assert_eq!(def(b"$( echo true"), None);
    assert_eq!(def(b"a$( echo true"), None);
}

/// The region jump must not over-consume: a word with no `$(` is split exactly
/// as before, and a `)` right behind a region is the tokenizer's own word.
#[test]
fn substitution_region_does_not_move_the_word_boundaries() {
    assert_eq!(def(b"f() { echo hi"), Some(b"f".to_vec()));
    // A glued `(` with a split closer is the accepted `f( ) {` form; the fully
    // split `f ( ) {` stays rejected by both layers (LESSONS, task #235).
    assert_eq!(def(b"f( ) { echo hi"), Some(b"f".to_vec()));
    assert_eq!(def(b"f ( ) { echo hi"), None, "split parens stay rejected");
    assert_eq!(def(b"$(echo a))x"), None, "the `)` is the next word");
    assert_eq!(name_len(b"a b"), None);
    assert_eq!(definer_len(b"f(", b"f(", 0), Some(2));
    // A `$` that is not a region opener is an ordinary name byte (the
    // name-validity rule is task #166's, so a change here is a deliberate edit).
    assert_eq!(def(b"$F() {"), Some(b"$F".to_vec()));
}

/// The region jump lands **just past the region's closing `)`**, so the word end
/// is the byte the tokenizer's word split lands on. Pinning the offsets, not just
/// the `None`s: the rule is stated over word offsets, and a jump aimed at the
/// region's own `(` (a `+` → `*` mutant of the start offset) re-enters the region
/// and claims the text behind it — invisible through `name_len`'s strictness
/// today, a live divergence from the tokenizer as soon as #166 loosens it.
#[test]
fn word_end_region_offsets() {
    assert_eq!(word_end(b"$(echo a))x"), 9);
    assert_eq!(word_end(b"$()x)"), 4);
    assert_eq!(word_end(b"$( echo a ) x)"), 11);
}
