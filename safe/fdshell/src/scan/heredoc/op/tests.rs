#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::{Operator, invalid_delimiter, operator_delim, untab, untab_body};
use crate::mask::MaskBit;
use sys::ShortCStr;

#[test]
fn operator_delim_attached_word_is_plain() {
    let line = b"<<WORD";
    assert_eq!(operator_delim(line, 0, 6), (false, b"WORD" as &[u8]));
}

#[test]
fn operator_delim_dash_marker_takes_the_word() {
    let line = b"<<-WORD";
    assert_eq!(operator_delim(line, 0, 7), (true, b"WORD" as &[u8]));
}

#[test]
fn operator_delim_dash_only_is_bare_marker() {
    let line = b"<<-";
    assert_eq!(operator_delim(line, 0, 3), (true, b"" as &[u8]));
}

#[test]
fn operator_delim_consumes_exactly_one_dash() {
    // `<<---`: one `-` is the marker, `--` is the delimiter.
    let line = b"<<---";
    assert_eq!(operator_delim(line, 0, 5), (true, b"--" as &[u8]));
}

#[test]
fn operator_delim_keeps_quotes_for_the_operator() {
    // The quotes are stripped by `Operator::new`, not by the byte rule.
    let line = b"<<\"Q\"";
    assert_eq!(operator_delim(line, 0, 5), (false, b"\"Q\"" as &[u8]));
}

#[test]
fn untab_strips_leading_tabs_only() {
    assert_eq!(untab(b"\tone"), b"one" as &[u8]);
    assert_eq!(untab(b"\t\ta\tb"), b"a\tb" as &[u8]);
    assert_eq!(untab(b"  sp"), b"  sp" as &[u8]);
    assert_eq!(untab(b"plain"), b"plain" as &[u8]);
    assert_eq!(untab(b""), b"" as &[u8]);
}

#[test]
fn untab_body_strips_leading_tabs_of_every_line() {
    assert_eq!(
        untab_body(b"\tone\n\t\ttwo\nthree\n").as_slice(),
        b"one\ntwo\nthree\n" as &[u8]
    );
    assert_eq!(untab_body(b"").as_slice(), b"" as &[u8]);
    // A tabs-only line becomes empty, keeping its terminating newline.
    assert_eq!(untab_body(b"\t\t\n").as_slice(), b"\n" as &[u8]);
    assert_eq!(untab_body(b"body\n\t\t\n").as_slice(), b"body\n\n" as &[u8]);
}

#[test]
fn operator_matches_respects_strip() {
    let plain = Operator::new(b"EOF", false, 0);
    assert!(plain.matches(b"EOF"));
    assert!(!plain.matches(b"\tEOF"), "the plain form never strips tabs");
    let dash = Operator::new(b"EOF", true, 0);
    assert!(dash.matches(b"EOF"));
    assert!(dash.matches(b"\tEOF"));
    assert!(dash.matches(b"\t\tEOF"));
    assert!(!dash.matches(b" xEOF"));
    assert!(!dash.matches(b"EOFx"));
}

#[test]
fn operator_body_bytes_respects_strip() {
    let plain = Operator::new(b"EOF", false, 0);
    assert_eq!(plain.body_bytes(b"\tone\n").as_slice(), b"\tone\n" as &[u8]);
    let dash = Operator::new(b"EOF", true, 0);
    assert_eq!(
        dash.body_bytes(b"\tone\n\t\ttwo\n").as_slice(),
        b"one\ntwo\n" as &[u8]
    );
}

#[test]
fn operator_new_strips_quotes_and_carries_flags() {
    let q = Operator::new(b"\"Q\"", true, 0);
    assert_eq!(q.delim, b"Q" as &[u8]);
    assert!(q.quoted);
    assert!(q.strip);
    let p = Operator::new(b"EOF", false, 0);
    assert_eq!(p.delim, b"EOF" as &[u8]);
    assert!(!p.quoted);
    assert!(!p.strip);
}

/// The delimiter word is a word: quote removal strips **every** quote byte,
/// not one surrounding pair, and any quoting marks the delimiter as quoted
/// (task #207 — the one-pair rule left `<<a'b'c`'s and `<<""Q"x"`'s bytes in).
#[test]
fn operator_new_strips_every_quote_byte() {
    for (raw, want) in [
        (b"a'b'c" as &[u8], b"abc" as &[u8]),
        (b"'x'y'z'", b"xyz"),
        (b"ab'c'", b"abc"),
        (b"'a'bc", b"abc"),
        (b"a\"b\"c", b"abc"),
        (b"\"\"Q\"x\"", b"Qx"),
        (b"a\"b c\"d", b"ab cd"),
        (b"\"a b\"", b"a b"),
        (b"'a <<b'", b"a <<b"),
        (b"\"a <<b\"", b"a <<b"),
        (b"'a\"b'", b"a\"b"),
        (b"\"a'b\"", b"a'b"),
    ] {
        let op = Operator::new(raw, false, 0);
        assert_eq!(
            op.delim,
            want,
            "raw={:?}",
            core::str::from_utf8(raw).unwrap()
        );
        assert!(op.quoted, "a quote byte means a quoted delimiter");
        assert!(!op.strip);
    }
}

/// The delimiter word's *text* keeps an escaped quote as data (the pair folds
/// in the strip pass), while the *pair itself* is a POSIX quote operator and
/// therefore **quotes** the word: `<<a\'b` delimits `a'b` with a literal body,
/// `<<E\OF` delimits `EOF` with a literal body (POSIX XCU 2.3, task #263).
#[test]
fn operator_new_folds_escapes_and_quotes_the_delimiter() {
    let op = Operator::new(b"a\\'b", false, 0);
    assert_eq!(op.delim, b"a'b" as &[u8]);
    assert!(op.quoted);
    let op = Operator::new(b"E\\OF", false, 0);
    assert_eq!(op.delim, b"EOF" as &[u8]);
    assert!(op.quoted);
    // A quoted pair survives: the delimiter is the literal `E\OF`.
    let op = Operator::new(b"\"E\\OF\"", false, 0);
    assert_eq!(op.delim, b"E\\OF" as &[u8]);
    assert!(op.quoted);
}

/// A trailing `\` escapes nothing, so it is no pair: the delimiter keeps its
/// backslash and the word stays unquoted. Kill test for "the flag is set by
/// any `\` byte" and for swapping the two arms of the fold's escape case.
#[test]
fn operator_new_a_trailing_backslash_is_not_a_pair() {
    let op = Operator::new(b"E\\", false, 0);
    assert_eq!(op.delim, b"E\\" as &[u8]);
    assert!(!op.quoted, "a lone trailing `\\` quotes nothing");
}

/// Every form of a folded pair quotes the delimiter, with and without the
/// `<<-` marker (the flag is derived in the fold, so `strip` cannot change it).
#[test]
fn operator_new_a_folded_pair_quotes_every_form() {
    for (raw, want) in [
        (b"\\E\\O\\F" as &[u8], b"EOF" as &[u8]),
        (b"E\\OF", b"EOF"),
        (b"\\\\$HOME", b"\\$HOME"),
        (b"\\$HOME", b"$HOME"),
        (b"EO\\F", b"EOF"),
    ] {
        for strip in [false, true] {
            let op = Operator::new(raw, strip, 0);
            assert_eq!(op.delim, want, "raw={:?}", raw);
            assert!(op.quoted, "raw={:?} strip={:?}", raw, strip);
            assert_eq!(op.strip, strip);
        }
    }
}

/// A quoted delimiter's terminator is the stripped word, byte-exactly.
#[test]
fn operator_matches_the_stripped_quoted_delimiter() {
    let op = Operator::new(b"a'b'c", false, 0);
    assert!(op.matches(b"abc"));
    assert!(
        !op.matches(b"a'b'c"),
        "the terminator line keeps its raw bytes"
    );
    let dash = Operator::new(b"'a'bc", true, 0);
    assert!(dash.matches(b"\tabc"));
    assert!(!dash.matches(b"\ta'bc"));
}

/// The bare-operand guard's operator-byte arm reads the operand's **quote
/// mask** (task #323): `&`, `<`, `>`, `%` make the word an operator only when
/// that byte is unquoted — POSIX #4.1 removes quotes after tokenization, so
/// `"&f"` is a plain word where `&f` is a syntax error. The mask is parallel
/// to the word text (the tokenizer contract); a missing bit pads to
/// *unquoted*, so an empty mask keeps the text-only rule. `""`, `;` and `|`
/// are mask-independent arms of `invalid_delimiter`. One positive row per
/// operator byte keeps the four arms killable.
#[test]
fn invalid_delimiter_gates_operator_bytes_on_the_quote_mask() {
    let (t, f) = (MaskBit::Quoted, MaskBit::Unquoted);
    for (word, mask, want) in [
        // Quoted leading byte: an ordinary word in bash (`> "&f"` opens `&f`).
        (b"&f".as_slice(), &[t][..], false),
        (b"<x", &[t], false),
        (b">x", &[t], false),
        (b"%x", &[t], false),
        (b"&f", &[t, f], false),
        // Unquoted leading byte: the operator (bash syntax errors, one row
        // per arm byte).
        (b"&f", &[f], true),
        (b"<x", &[f], true),
        (b">x", &[f], true),
        (b"%x", &[f], true),
        (b"&f", &[f, t], true),
        // A missing bit pads to unquoted: the text-only rule.
        (b"&f", &[f][..0], true),
        (b"%x", &[f][..0], true),
        // Mask-independent arms and plain words.
        (b"", &[f], true),
        (b";", &[f], true),
        (b"|", &[f], true),
        (b"abc", &[f], false),
        (b"x&f", &[f], false),
        (b"--", &[f], false),
    ] {
        let word = ShortCStr::from_vec(word.to_vec()).unwrap();
        assert_eq!(
            invalid_delimiter(&word, mask),
            want,
            "word={:?} mask={:?}",
            word,
            mask
        );
    }
}
