#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use crate::error::parse::ParseError;
use crate::parse::ParsedLine;
use crate::parse::array_literal::{ArrayAssign, ArrayWord};
use crate::parse::parse;
use alloc::vec;
use alloc::vec::Vec;
use sys::Origin;
use sys::{Position, ScriptText, ShortCStr};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

fn s(b: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(b.to_vec()).unwrap()
}

/// The array arm's shape of a statement: one `(name, append, element texts)`
/// row per `name=(…)` word, in the order written.
fn shape(line: &[u8]) -> Vec<(ShortCStr, bool, Vec<ShortCStr>)> {
    let parsed = parse(&stext(line)).unwrap();
    let ParsedLine::AssignArray(assigns) = parsed else {
        panic!("the array arm must own this statement: {line:?}");
    };
    shape_of(&assigns)
}

fn shape_of(assigns: &[ArrayAssign]) -> Vec<(ShortCStr, bool, Vec<ShortCStr>)> {
    assigns
        .iter()
        .map(|assign| {
            (
                assign.name.clone(),
                assign.append,
                assign
                    .elements
                    .iter()
                    .map(|word| word.text.clone())
                    .collect(),
            )
        })
        .collect()
}

/// The words of one region, whole: mask length, spans and the quoted flag
/// included (the parallel plumbing the runtime expands word by word).
fn words(line: &[u8]) -> Vec<ArrayWord> {
    let parsed = parse(&stext(line)).unwrap();
    let ParsedLine::AssignArray(assigns) = parsed else {
        panic!("the array arm must own this statement: {line:?}");
    };
    assigns.first().unwrap().elements.clone()
}

#[test]
fn one_word_region() {
    assert_eq!(shape(b"m=(1)"), vec![(s(b"m"), false, vec![s(b"1")])]);
}

#[test]
fn a_spaced_region_keeps_every_word() {
    assert_eq!(
        shape(b"a=(1 2)"),
        vec![(s(b"a"), false, vec![s(b"1"), s(b"2")])]
    );
}

#[test]
fn a_quoted_element_keeps_its_bytes() {
    // The tokenizer's word is `a=(b c` — the quote bytes are consumed, the
    // element keeps its own mask (the parallel plumbing of `parse.rs`).
    assert_eq!(
        shape(b"a=(\"b c\")"),
        vec![(s(b"a"), false, vec![s(b"b c")])]
    );
    let element = words(b"a=(\"b c\")").remove(0);
    assert_eq!(element.mask.len(), 3, "every element byte is masked");
    assert!(
        element.mask.iter().all(|m| m.quoted()),
        "the element was written inside quotes"
    );
}

#[test]
fn an_empty_region_has_no_element() {
    // bash: `m=(); echo "${#m[@]}"` → `0` — the `(`-word carried no bytes and
    // no quote material, so it contributes nothing.
    assert_eq!(shape(b"m=()"), vec![(s(b"m"), false, Vec::new())]);
    assert_eq!(shape(b"a=( )"), vec![(s(b"a"), false, Vec::new())]);
}

#[test]
fn a_quoted_empty_word_is_an_element() {
    // bash: `a=(""); echo "${#a[@]}"` → `1`. The region's first word has no
    // bytes of its own, so the zero-byte span (task #321) is its evidence.
    let elements = words(b"a=(\"\")");
    assert_eq!(elements.len(), 1);
    assert_eq!(elements[0].text, ShortCStr::new());
    assert_eq!(
        elements[0].spans,
        vec![0],
        "the `\"\"` pair closed at the start"
    );
}

#[test]
fn an_append_word_marks_the_name() {
    // `m+=(3)`: the sign is the append flag, not a name byte.
    assert_eq!(shape(b"m+=(3)"), vec![(s(b"m"), true, vec![s(b"3")])]);
}

#[test]
fn two_regions_of_one_statement_both_persist() {
    // bash: `a=(1) b=(2); echo "${a[@]}-${b[@]}"` → `1-2`.
    assert_eq!(
        shape(b"a=(1) b=(2)"),
        vec![
            (s(b"a"), false, vec![s(b"1")]),
            (s(b"b"), false, vec![s(b"2")]),
        ]
    );
}

#[test]
fn a_region_ends_at_the_separator_token() {
    // A quoted `)` is a word, never the region's end (the byte/token agreement
    // task #300 pinned); an escaped one is a word too.
    assert_eq!(shape(b"a=(\")\")"), vec![(s(b"a"), false, vec![s(b")")])]);
    assert_eq!(shape(b"a=(\\))"), vec![(s(b"a"), false, vec![s(b"\\)")])]);
}

#[test]
fn an_unclosed_region_is_the_arm_s_own_report() {
    // The region is bounded by the statement; bash rejects the shape at rc 2
    // and fdshell reports its own message at rc 1 (task #422).
    for line in [&b"a=(1"[..], &b"a=(1 2 ; echo x"[..], &b"a=(\"x\""[..]] {
        let err = parse(&stext(line)).err().unwrap();
        assert!(
            matches!(
                err.current_context(),
                ParseError::ArrayLiteralMissingCloseParen
            ),
            "{line:?}: {err}"
        );
    }
}

#[test]
fn a_scoped_prefix_stays_the_command_walk_s() {
    // `a=(1 2) echo hi` is a command with an assignment prefix in bash (it prints
    // `hi`), so the array arm must not claim the statement; the divergence is
    // pinned in the integration table.
    let parsed = parse(&stext(b"a=(1 2) echo hi")).unwrap();
    assert!(
        matches!(parsed, ParsedLine::Cmd(_)),
        "the command walk owns it"
    );
}

#[test]
fn a_mixed_statement_stays_the_command_walk_s() {
    // An array word mixed with a plain assignment word is not "all array words",
    // so the arm does not claim it and the ordinary walk answers — the
    // divergence pinned in the integration table (bash `a=(1) b=2` → `1-2`).
    let parsed = parse(&stext(b"a=(1) b=2")).unwrap();
    assert!(
        matches!(parsed, ParsedLine::Cmd(_)),
        "the command walk owns it"
    );
}
