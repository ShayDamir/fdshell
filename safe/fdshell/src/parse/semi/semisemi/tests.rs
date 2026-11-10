#![allow(clippy::unwrap_used)]
use super::*;
use crate::parse::token::tokenize;
use alloc::vec;
use alloc::vec::Vec;

/// The byte positions at which the scanner's reader answers on `line` with
/// `state` (the positions the walk consults; the reader is the predicate, the
/// walk settles `state` before consulting it).
fn byte_pair(line: &[u8], state: &ScanState) -> Vec<usize> {
    (0..line.len())
        .filter(|&i| pair_at_byte(line, i, state))
        .collect()
}

/// [`first_stray`] on the whole line's token list, tokenized as the parse layer
/// sees it.
fn stray(line: &[u8]) -> Option<usize> {
    let tokens = tokenize(line).unwrap();
    first_stray(&tokens, line)
}

/// [`first_stray`] on the tokens of `line` between the words `open` and `close`
/// (the body slice a builder passes), tokenized as the parse layer sees it.
fn stray_between(line: &[u8], open: &[u8], close: &[u8]) -> Option<usize> {
    let tokens = tokenize(line).unwrap();
    let o = tokens
        .iter()
        .position(|Token { word: t, .. }| t.eq_bytes(open))
        .unwrap();
    let c = tokens
        .iter()
        .rposition(|Token { word: t, .. }| t.eq_bytes(close))
        .unwrap();
    first_stray(tokens.get(o + 1..c).unwrap_or(&[]), line)
}

/// The glue arm: a glued pair answers at its head byte only, and the token
/// reader answers at the head token.
#[test]
fn a_glued_pair_answers_at_its_head() {
    assert_eq!(byte_pair(b"echo a;;echo b", &ScanState::new()), vec![6]);
    assert_eq!(stray(b"echo a;;echo b"), Some(2));
}

/// The spaced arm: a space gap is the same pair at both granularities
/// (decision D2's non-glued spellings).
#[test]
fn a_spaced_gap_is_the_same_pair() {
    assert_eq!(byte_pair(b"echo a ; ; echo b", &ScanState::new()), vec![7]);
    assert_eq!(stray(b"echo a ; ; echo b"), Some(2));
}

/// The quote arm: the walk's [`ScanState`] settles a quoted `";;"` before the
/// predicate is consulted, and at the token level the raw-span gate of
/// [`crate::parse::Token::span_word`] is the same answer (task #336's rule read at both levels).
#[test]
fn a_quoted_pair_is_word_data() {
    let mut state = ScanState::new();
    state.in_quote = true;
    assert_eq!(byte_pair(b"echo a \";;\" b", &state), vec![]);
    assert_eq!(stray(b"echo a \";\" \";\" b"), None);
}

/// The escape arm: neither `;` byte of `\;\;` is a bare one (the walk consumes
/// escape pairs), and an escape is word data at the token level too.
#[test]
fn an_escaped_pair_is_not_a_pair() {
    assert_eq!(
        byte_pair(b"echo a \\;\\; echo b", &ScanState::new()),
        vec![]
    );
    assert_eq!(stray(b"echo a \\;\\; echo b"), None);
}

/// The newline-gap arm (byte level): a newline gap is no pair — the `;\n;`
/// family belongs to task #487, pinned as must not fire.
#[test]
fn a_newline_gap_is_no_pair() {
    assert_eq!(byte_pair(b"echo a ;\n; echo b", &ScanState::new()), vec![]);
    // The tokenizer writes the newline's `;` as a separator word, so the token
    // reader needs the same gap rule: glued-behind-a-newline and newline-between
    // are both no pair (#487's family, measured at both levels).
    assert_eq!(stray(b"echo a ;\n; echo b"), None);
    assert_eq!(stray(b"echo a ;\n"), None);
    assert_eq!(stray(b"echo a ||\n; echo b"), None);
    // A body of the pipeline test's shape: the two real `;`s sit at different
    // statement levels and the newline word sits between the walk's path.
    assert_eq!(stray(b"cat <<T; break;\nbody\nT"), None);
}

/// The `;&`/`;;&` arm: the terminator kinds are excluded by
/// [`terminator::kind_at`] != `Stop` at the token level, and the byte reader
/// never sees them as a pair (`;&` has no second `;` byte; a `&` glued behind
/// the glued pair is the `;;&` kind, not this rule's pair).
#[test]
fn the_other_terminator_kinds_are_excluded() {
    assert_eq!(byte_pair(b"echo a ;& echo b", &ScanState::new()), vec![]);
    assert_eq!(byte_pair(b"echo a ;;& echo b", &ScanState::new()), vec![]);
    assert_eq!(stray(b"echo a ;& echo b"), None);
    assert_eq!(stray(b"echo a ;;& echo b"), None);
}

/// The nested-case arm: the shared walk pairs the nested `case`, so the `;;`
/// inside it sits at depth 1 and the walk walks past it.
#[test]
fn a_pair_inside_a_nested_case_is_skipped() {
    assert_eq!(
        stray_between(b"f() { case b in b) echo y;; esac }", b"{", b"}"),
        None
    );
}

/// The leading arm: a pair inside the slice's leading separator run (what
/// `trim_body` absorbs) is the accepted spelling, not a stray one; a pair with
/// kept content before it is.
#[test]
fn a_leading_pair_stays_as_measured() {
    assert_eq!(stray_between(b"bar() { ;; :; }", b"{", b"}"), None);
    assert_eq!(stray_between(b"bar() { : ;; }", b"{", b"}"), Some(1));
}

/// The parse layer's reader answers at the head token of a body's pair (the
/// `if true; then echo a ;; fi` shape, the plan's scope row).
#[test]
fn a_trailing_pair_in_a_body_reports() {
    assert_eq!(
        stray_between(b"if true; then echo a ;; fi", b"then", b"fi"),
        Some(2)
    );
}

#[test]
fn a_newline_separator_body_stays_silent() {
    assert_eq!(
        stray(b"echo a | while :; do cat <<T; break;\nbody\nT\ndone"),
        None
    );
    assert_eq!(stray(b"echo a | f() { echo b; }; f"), None);
    assert_eq!(stray(b"while false; do echo a || ;\ndone"), None);
}

/// Each guard operand blanks a real pair **on its own** (rule 8). The rows above
/// settle `state` through the walk; a direct probe per operand measures each `||`
/// and the `> 0` — with only one operand set, the guard must still blank the
/// `;;` at byte 1 of `a;;b`.
#[test]
fn each_guard_operand_alone_blanks_the_pair() {
    let mut s = ScanState::new();
    s.in_single = true;
    assert_eq!(byte_pair(b"a;;b", &s), vec![]);
    let mut s = ScanState::new();
    s.in_backtick = true;
    assert_eq!(byte_pair(b"a;;b", &s), vec![]);
    let mut s = ScanState::new();
    s.paren_depth = 1;
    assert_eq!(byte_pair(b"a;;b", &s), vec![]);
    assert_eq!(byte_pair(b"a;;b", &ScanState::new()), vec![1]);
}
