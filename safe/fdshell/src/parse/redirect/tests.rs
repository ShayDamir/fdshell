//! The operator-quoting gate of the `>`-family parsers (task #264): POSIX
//! XCU 2.9.1 builds a redirection out of **unquoted** bytes — the io-number
//! digits and every operator byte. A quote byte ends the operator and begins
//! the word, so `2'>'g2` is the argument `2>g2` and no file is created; a
//! quoted digit run is no io-number, so `'2'>f` moves the operator's default
//! fd 1. Masks come from the tokenizer, never from hand-built vectors
//! (LESSONS: the tokenizer's per-byte mask, not a re-derived quote scan).

#![allow(clippy::unwrap_used)]
use crate::mask::MaskBit;
use crate::mask::MaskBit::Unquoted;
use crate::mask::tests::runs;
use crate::parse::bg_redirect::parse_bg_redirect;
use crate::parse::redirect::{parse_redirect, quoted_bytes, token_fd};
use crate::parse::token::tokenize_statement;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

/// The operator token (token 1) of a probe line: its text, mask, and the
/// `>`/`<` byte position — the prefix and the gate's `from` bound.
fn op_token(line: &[u8]) -> (ShortCStr, Vec<MaskBit>, usize) {
    let tokens = tokenize_statement(line).unwrap();
    let (t, _s, _e, _fq, mask, _spans) = tokens.get(1).unwrap();
    let bytes = t.as_bytes().unwrap();
    let op_pos = bytes
        .iter()
        .position(|&b| b == b'>' || b == b'<')
        .unwrap_or(t.len());
    (t.clone(), mask.clone(), op_pos)
}

/// `parse_redirect` on token 1 of a probe line.
fn redirect_of(line: &[u8]) -> Option<(RedirectDef, usize)> {
    let tokens = tokenize_statement(line).unwrap();
    parse_redirect(&tokens, 1).unwrap()
}

/// `parse_bg_redirect` on token 1: the redirect list only (the pidvar arm of
/// a quoted word must report *no* job, the #250 lesson).
fn bg_of(line: &[u8]) -> Option<(Vec<RedirectDef>, Option<ShortCStr>, bool)> {
    let tokens = tokenize_statement(line).unwrap();
    parse_bg_redirect(&tokens, 1)
        .unwrap()
        .map(|(r, _n)| (r.redirects, r.pidvar, r.bg_force))
}

fn assert_path(line: &[u8], export_to: i32, direction: RedirectDirection, path: &str) {
    let (def, _consumed) = redirect_of(line).unwrap_or_else(|| panic!("{line:?} must redirect"));
    assert_eq!(def.export_to, export_to, "{line:?}");
    assert_eq!(def.direction, direction, "{line:?}");
    let want = ShortCStr::from_vec(path.as_bytes().to_vec()).unwrap();
    match def.source {
        RedirectSource::Path { path: p, .. } => assert_eq!(p, want, "{line:?}"),
        ref s => panic!("{line:?}: source {s:?} is not a path"),
    }
}

fn assert_argument(line: &[u8]) {
    assert!(
        redirect_of(line).is_none(),
        "{line:?} must stay an argument"
    );
}

/// A quoted digit run is no io-number: the operator takes its default fd
/// (POSIX XCU 2.9.1: `io-number` is a run of *unquoted* digits). `2` proves
/// the default arm, `a` and `\2` keep the "non-digit prefix is not a
/// redirect" rule (fdshell never splits a word at an operator byte).
#[test]
fn token_fd_reads_only_unquoted_digits() {
    let (t, mask, op) = op_token(b"echo \"2\">f");
    assert_eq!(token_fd(&t.get(..op).unwrap(), &mask, b'>'), Some(1));
    let (t, mask, op) = op_token(b"echo 2>f");
    assert_eq!(token_fd(&t.get(..op).unwrap(), &mask, b'>'), Some(2));
    let (t, mask, op) = op_token(b"echo 2<f");
    assert_eq!(token_fd(&t.get(..op).unwrap(), &mask, b'<'), Some(2));
    let (t, mask, op) = op_token(b"echo \"2\"<f");
    assert_eq!(token_fd(&t.get(..op).unwrap(), &mask, b'<'), Some(0));
    let (t, mask, op) = op_token(b"echo <f");
    assert_eq!(token_fd(&t.get(..op).unwrap(), &mask, b'<'), Some(0));
    let (t, mask, op) = op_token(b"echo a>f");
    assert_eq!(token_fd(&t.get(..op).unwrap(), &mask, b'>'), None);
    // The escape pair keeps its `\` in the token text, so the prefix is not
    // a digit run at all (LESSONS: `\if` is no keyword, `\&>` no operator).
    let (t, mask, op) = op_token(b"echo \\2>f");
    assert_eq!(token_fd(&t.get(..op).unwrap(), &mask, b'>'), None);
}

/// The gate's slice arms: a quoted bit *inside* the operator range fires;
/// a mask shorter than the range pads to unquoted, the house idiom.
#[test]
fn quoted_bytes_slices_and_pads() {
    let (_t, mask, _op) = op_token(b"echo 2>\"&\"1");
    assert!(!quoted_bytes(&mask, 1, 2), "the unquoted `>` byte");
    assert!(quoted_bytes(&mask, 2, 3), "the quoted `&` continuation");
    assert!(
        !quoted_bytes(&mask, 4, 9),
        "a slice past the mask pads to unquoted"
    );
    assert!(!quoted_bytes(&[], 0, 1), "the empty mask is unquoted");
}

/// R1: a quoted first operator byte ends the operator — every shape of the
/// family is an ordinary argument, exactly where bash prints the literal
/// (measured 2026-10-20 against bash 5.3.9, each shell in its own dir).
#[test]
fn quoted_first_operator_byte_is_an_argument() {
    for line in [
        b"echo 2'>'g2".as_slice(),
        b"echo 1\">\"f".as_slice(),
        b"echo 1\">|\"f".as_slice(),
        b"echo 1'>>'g".as_slice(),
        b"echo 1'<>'g".as_slice(),
        b"echo 2'>&1'".as_slice(),
        b"echo '<'f".as_slice(),
        b"echo '>'f".as_slice(),
        b"echo '>>'f".as_slice(),
        b"echo '>|'f".as_slice(),
    ] {
        assert_argument(line);
    }
}

/// R1 on the bare form: `1'>'` is the argument `1>`, not a bare operator
/// bailing `invalid redirect` on its missing operand (bash rc 0).
#[test]
fn bare_quoted_operator_is_an_argument() {
    assert_argument(b"echo 1'>'");
    // The unquoted bare operator stays an operator (takes the next token).
    let (def, consumed) = redirect_of(b"echo > f").unwrap();
    assert_eq!((def.export_to, consumed), (1, 1));
}

/// R1's continuation clause: a quoted byte after the first operator byte
/// ends the operator there, and the rest is the operand word — `2> "&" 1`
/// opens the **file** `&1` (fd 2), `> "&" f` opens `&f` (fd 1), as bash does.
#[test]
fn quoted_continuation_byte_makes_the_operand_a_path() {
    assert_path(b"echo 2>\"&\"1", 2, RedirectDirection::Write, "&1");
    assert_path(b"echo >\"&\"f", 1, RedirectDirection::Write, "&f");
    // The `>>` twin: `> ">" f`-shaped word — quoted second byte, so a `>`
    // operator with the path `>f` (bash opens the file `>f`, measured).
    assert_path(b"echo >\">\"f", 1, RedirectDirection::Write, ">f");
    // The unquoted `&` keeps the dup arm.
    let (def, _c) = redirect_of(b"echo 2>&1").unwrap();
    assert_eq!(def.export_to, 2);
    assert!(matches!(def.source, RedirectSource::Dup(1)));
    // A quoted dup operand is the plain fd word, unchanged (task #323 side).
    let (def, _c) = redirect_of(b"echo 2>&\"1\"").unwrap();
    assert_eq!(def.export_to, 2);
    assert!(matches!(def.source, RedirectSource::Dup(1)));
}

/// R2: quoted digits are no io-number, so the operator moves its default
/// descriptor; the redirect itself stands because its bytes are unquoted.
#[test]
fn quoted_digits_take_the_default_fd() {
    assert_path(b"echo '1'>g1", 1, RedirectDirection::Write, "g1");
    assert_path(b"echo \"2\">g", 1, RedirectDirection::Write, "g");
    assert_path(b"echo '1'>|f", 1, RedirectDirection::Clobber, "f");
    assert_path(b"echo \"2\">|f", 1, RedirectDirection::Clobber, "f");
    // The dup arm reads the same io-number rule: fd 1 (default) dup 1.
    let (def, _c) = redirect_of(b"echo '1'>&1").unwrap();
    assert_eq!(def.export_to, 1);
    assert!(matches!(def.source, RedirectSource::Dup(1)));
}

/// The unquoted family is byte-for-byte unchanged (the regression net that
/// keeps `2>&abc`-style arguments and every real redirect parsing).
#[test]
fn unquoted_shapes_are_unchanged() {
    assert_path(b"echo >f", 1, RedirectDirection::Write, "f");
    assert_path(b"echo 3>>f", 3, RedirectDirection::Append, "f");
    assert_path(b"echo <f", 0, RedirectDirection::Read, "f");
    assert_path(b"echo 3<>f", 3, RedirectDirection::Rw, "f");
    // A `%`-prefixed word keeps the capture/clobber rules of its own layer.
    let (def, _c) = redirect_of(b"echo 3>%x").unwrap();
    assert_eq!(def.export_to, 3);
    assert!(matches!(def.source, RedirectSource::Var(_)));
    // Not redirects at all: a non-digit prefix, no operator byte.
    assert_argument(b"echo a>f");
    assert_argument(b"echo 1\"|\"f");
}

/// R1 on the `&>` family: the `&` and the `>` (and the `&`/`|` of the
/// pidvar arms) are operator bytes, so any quote among them makes the word
/// an argument. The pidvar arms must record **no job** (the #250 lesson).
#[test]
fn quoted_combined_operator_bytes_are_arguments() {
    for line in [
        b"echo '&>q.txt'".as_slice(),
        b"echo '&>'f".as_slice(),
        b"echo '&>>'f".as_slice(),
        b"echo '&>'%v".as_slice(),
        b"echo '&>&p'".as_slice(),
        b"echo '&>|&p'".as_slice(),
    ] {
        let bg = bg_of(line);
        assert!(bg.is_none(), "{line:?}: the quoted word opens no redirect");
    }
    // The unquoted combined operator is unchanged.
    let (redirects, pidvar, bg_force) = bg_of(b"echo &>f").unwrap();
    assert_eq!(redirects.len(), 1);
    assert!(pidvar.is_none() && !bg_force);
    // A quoted *path* byte is operand data, not the gate (unchanged).
    let (redirects, _, _) = bg_of(b"echo &>\"f\"").unwrap();
    assert_eq!(redirects.len(), 1);
}

/// The 3-byte operator arms (`&>>`, `&>&name`, `&>|&name`) extend the gate to
/// the **third** byte: a word quoting only that byte opens nothing and
/// records no job (the pidvar arms are the kill: a shortened gate turns
/// these words into background jobs). fdshell's word rule (the word-split
/// family owns bash's split of a leading `&` control operator).
#[test]
fn quoted_third_operator_byte_of_the_three_byte_forms_is_an_argument() {
    for line in [
        b"echo &\">\">f".as_slice(),
        b"echo &\">\"&p",
        b"echo &>\"&\"p",
        b"echo &>\"|\"&p",
    ] {
        assert!(
            bg_of(line).is_none(),
            "{line:?}: no redirect from a quoted operator"
        );
    }
}

/// The dup arm's `&` gate with a multi-digit io-number: `12> "&" x` ends the
/// operator at the quoted `&`, and fd 12 opens the **file** `&x` — the file
/// bash's `12> "&" x` creates too (measured 2026-10-20, parity).
#[test]
fn quoted_dup_ampersand_after_a_multi_digit_io_number_is_a_path() {
    assert_path(b"echo 12>\"&\"x", 12, RedirectDirection::Write, "&x");
}

/// The target word's quoting **quadruple** (task #278): the opener expands the
/// target as a word, so the bare form hands over the operand token's own mask,
/// zero-byte quote spans and `quoted` flag, and the attached form slices the
/// mask and **shifts** the spans at the operator offset. An unshifted span
/// index is invisible on a `~`-free word, so every probe carries a `~` gate.
#[test]
fn bare_target_carries_the_operand_quoting() {
    assert_target(
        b"echo x > \"a b\"",
        RedirectSource::path_word(c"a b", runs(&[(true, 3)]), Vec::new(), true),
    );
    assert_target(
        b"echo x > f",
        RedirectSource::path_word(c"f", vec![Unquoted], Vec::new(), false),
    );
    // The empty pair closed at the word's index 1, and POSIX counts it inside
    // the tilde-prefix, so `> ~""` is the literal `~` (bash's file `~`).
    assert_target(
        b"echo x > ~\"\"",
        RedirectSource::path_word(c"~", vec![Unquoted], vec![1], true),
    );
    assert_target(
        b"echo x > ~$n",
        RedirectSource::path_word(c"~$n", runs(&[(false, 3)]), Vec::new(), false),
    );
}

/// The attached arms: the suffix word, its mask slice and its shifted spans
/// must agree, and the mask slice is not the whole token's. `f$(echo y)` is
/// the substitution case: every byte arrives **unquoted**, the rule bash shows
/// by creating the file `f` for `> "$(echo f)"` while `'$(echo f)'` stays
/// literal.
#[test]
fn attached_target_slices_and_shifts_at_the_operator_offset() {
    assert_target(
        b"echo x >\"a b\"",
        RedirectSource::path_word(c"a b", runs(&[(true, 3)]), Vec::new(), true),
    );
    // The token's pair is at index 2 (the `~` is the token's second byte); the
    // suffix word starts one byte in, so it must arrive as index 1.
    assert_target(
        b"echo x >~\"\"",
        RedirectSource::path_word(c"~", vec![Unquoted], vec![1], true),
    );
    assert_target(
        b"echo x >f$(echo y)",
        RedirectSource::path_word(c"f$(echo y)", runs(&[(false, 10)]), Vec::new(), false),
    );
    assert_target(
        b"echo x >| f",
        RedirectSource::path_word(c"f", vec![Unquoted], Vec::new(), false),
    );
    // The `&>` family's two arms: attached and bare.
    assert_target(
        b"echo x &>~\"\"",
        RedirectSource::path_word(c"~", vec![Unquoted], vec![1], true),
    );
    assert_target(
        b"echo x &> \"a b\"",
        RedirectSource::path_word(c"a b", runs(&[(true, 3)]), Vec::new(), true),
    );
}

/// The target source of a probe line's operator token (token **2** of every
/// probe below, `echo x <op>…`): the `>`-family parser, falling back to the
/// `&>`-family one (one helper for both families' quadruples).
fn target_of(line: &[u8]) -> RedirectSource {
    let tokens = tokenize_statement(line).unwrap();
    if let Some((def, _n)) = parse_redirect(&tokens, 2).unwrap() {
        return def.source;
    }
    parse_bg_redirect(&tokens, 2)
        .unwrap()
        .unwrap()
        .0
        .redirects
        .remove(0)
        .source
}

fn assert_target(line: &[u8], want: RedirectSource) {
    assert_eq!(target_of(line), want, "{line:?}");
}
