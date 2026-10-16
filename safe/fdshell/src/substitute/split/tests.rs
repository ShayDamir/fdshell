#![allow(clippy::unwrap_used)]

use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

use super::split_word;
use crate::substitute::origin::Origin;

fn scs(text: &str) -> ShortCStr {
    ShortCStr::from_vec(text.as_bytes().to_vec()).unwrap()
}

fn split(word: &str, ifs: &str) -> Vec<Vec<u8>> {
    // `Origin::all`: every byte came from an expansion — today's master
    // behavior, so the pre-task-#222 pins keep their expectations.
    split_word(&scs(word), &[], &scs(ifs), &Origin::all(word.len()))
        .unwrap()
        .iter()
        .map(|(s, _)| s.as_bytes().unwrap().to_vec())
        .collect()
}

/// Split with a per-byte quote mask: `true` marks IFS-protected bytes.
fn split_masked(word: &str, mask: &[bool], ifs: &str) -> Vec<Vec<u8>> {
    split_word(&scs(word), mask, &scs(ifs), &Origin::all(word.len()))
        .unwrap()
        .iter()
        .map(|(s, _)| s.as_bytes().unwrap().to_vec())
        .collect()
}

/// Split with an explicit provenance: `splittable` marks the expansion
/// bytes, `boundaries` the `$@` positional starts (unquoted).
fn split_origin(word: &str, splittable: &[usize], ifs: &str) -> Vec<Vec<u8>> {
    split_origin_q(word, splittable, &[], ifs)
}

fn split_origin_q(
    word: &str,
    splittable: &[usize],
    boundaries: &[usize],
    ifs: &str,
) -> Vec<Vec<u8>> {
    let mut origin = Origin::new();
    for &i in splittable {
        origin.record(i, i + 1);
    }
    for &i in boundaries {
        origin.boundary(i, false);
    }
    origin.close(word.len());
    split_word(&scs(word), &[], &scs(ifs), &origin)
        .unwrap()
        .iter()
        .map(|(s, _)| s.as_bytes().unwrap().to_vec())
        .collect()
}

/// Boundaries of a **quoted** `$@`: the positional bytes carry their quote
/// mask bit, and the boundary keeps an empty field at the word end.
fn split_origin_quoted(word: &str, boundaries: &[usize], ifs: &str) -> Vec<Vec<u8>> {
    let mut origin = Origin::new();
    for &i in boundaries {
        origin.boundary(i, true);
    }
    origin.close(word.len());
    let mask = vec![true; word.len()];
    split_word(&scs(word), &mask, &scs(ifs), &origin)
        .unwrap()
        .iter()
        .map(|(s, _)| s.as_bytes().unwrap().to_vec())
        .collect()
}

#[test]
fn default_ifs_splits_on_space_runs() {
    assert_eq!(
        split("a b c", " \t\n"),
        vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]
    );
}

#[test]
fn default_ifs_collapses_and_trims() {
    assert_eq!(
        split("  a \t b\n ", " \t\n"),
        vec![b"a".to_vec(), b"b".to_vec()]
    );
}

#[test]
fn default_ifs_keeps_single_word_intact() {
    assert_eq!(split("abc", " \t\n"), vec![b"abc".to_vec()]);
}

#[test]
fn empty_word_yields_no_fields() {
    assert_eq!(split("", " \t\n"), Vec::<Vec<u8>>::new());
    assert_eq!(split("   ", " \t\n"), Vec::<Vec<u8>>::new());
}

#[test]
fn empty_ifs_disables_splitting() {
    assert_eq!(split("a b", ""), vec![b"a b".to_vec()]);
}

#[test]
fn non_whitespace_ifs_delimits_fields() {
    assert_eq!(split("a:b", ":"), vec![b"a".to_vec(), b"b".to_vec()]);
}

#[test]
fn non_whitespace_ifs_keeps_empty_fields() {
    assert_eq!(
        split("a::b", ":"),
        vec![b"a".to_vec(), b"".to_vec(), b"b".to_vec()]
    );
}

#[test]
fn non_whitespace_ifs_elides_trailing_empty_field() {
    // A delimiter at the very end of the word produces no trailing empty
    // field (bash: `IFS=,; set -- $(printf "a,")` is one positional).
    assert_eq!(split("a:", ":"), vec![b"a".to_vec()]);
    assert_eq!(split("a,,", ","), vec![b"a".to_vec(), b"".to_vec()]);
}

#[test]
fn leading_empty_field_survives_trailing_elision() {
    // Leading empties are not elided: the lone delimiter of `:` delimits the
    // leading empty field, and the word end produces no second one (bash:
    // `IFS=:; set -- $(printf ":")` → `$#` = 1).
    assert_eq!(split(":", ":"), vec![b"".to_vec()]);
    assert_eq!(split(",,", ","), vec![b"".to_vec(), b"".to_vec()]);
}

#[test]
fn non_whitespace_ifs_keeps_leading_empty_field() {
    assert_eq!(split(":a", ":"), vec![b"".to_vec(), b"a".to_vec()]);
}

#[test]
fn delimiter_then_trailing_whitespace_merges() {
    // A non-whitespace delimiter at the end of a field merges with the
    // whitespace run that follows it: the field is not flushed and the
    // delimiter is trimmed with the run (`IFS=" ,": "a, "` is `a`).
    assert_eq!(split("a, ", " ,"), vec![b"a".to_vec()]);
    assert_eq!(split("a,, ", " ,"), vec![b"a".to_vec(), b"".to_vec()]);
    assert_eq!(split("a,   ", " ,"), vec![b"a".to_vec()]);
    // `IFS="a "`: `a` delimits, the space is the whitespace byte.
    assert_eq!(split("xaya ", "a "), vec![b"x".to_vec(), b"y".to_vec()]);
    assert_eq!(split("xaa ", "a "), vec![b"x".to_vec(), b"".to_vec()]);
}

#[test]
fn mixed_ifs_whitespace_terminates_field() {
    assert_eq!(split("a: b", " :"), vec![b"a".to_vec(), b"b".to_vec()]);
}

#[test]
fn mixed_ifs_whitespace_after_non_ws_keeps_empty() {
    assert_eq!(
        split("a :b", " :"),
        vec![b"a".to_vec(), b"".to_vec(), b"b".to_vec()]
    );
}

#[test]
fn masked_ifs_whitespace_never_splits() {
    // `x"a b"c`: the quoted space is data, the word stays whole.
    assert_eq!(
        split_masked("xa bc", &[false, true, true, true, false], " \t\n"),
        vec![b"xa bc".to_vec()]
    );
}

#[test]
fn masked_non_whitespace_ifs_never_delimits() {
    assert_eq!(
        split_masked("a:b", &[false, true, false], ":"),
        vec![b"a:b".to_vec()]
    );
}

#[test]
fn split_fields_carry_their_mask() {
    // `a"*"b c` (the quoted `*` is masked): each field keeps its span's bits.
    let pairs = split_word(
        &ShortCStr::from_vec(b"a*b c".to_vec()).unwrap(),
        &[false, true, false, false, false],
        &ShortCStr::from_vec(" \t\n".as_bytes().to_vec()).unwrap(),
        &Origin::all(5),
    )
    .unwrap();
    assert_eq!(
        pairs
            .iter()
            .map(|(s, m)| (s.as_bytes().unwrap().to_vec(), m.clone()))
            .collect::<Vec<_>>(),
        vec![
            (b"a*b".to_vec(), vec![false, true, false]),
            (b"c".to_vec(), vec![false]),
        ]
    );
}

#[test]
fn masked_leading_trailing_whitespace_is_kept() {
    // Quoted spaces at the word edges are not trimmed.
    assert_eq!(
        split_masked(" a ", &[true, false, true], " \t\n"),
        vec![b" a ".to_vec()]
    );
}

#[test]
fn whitespace_run_stops_at_protected_byte() {
    // `a" "  b`: quoted space kept, the unquoted run after it still splits.
    assert_eq!(
        split_masked("a   b", &[false, true, false, false, false], " \t\n"),
        vec![b"a ".to_vec(), b"b".to_vec()]
    );
}

#[test]
fn protected_byte_inside_run_is_data() {
    // `a "  b"`: unquoted space delimits, the quoted run is one field.
    assert_eq!(
        split_masked("a   b", &[false, false, true, true, false], " \t\n"),
        vec![b"a".to_vec(), b"  b".to_vec()]
    );
}

// Task #222: only expansion bytes delimit. The provenance marks which
// bytes an expansion produced; literal bytes are always content.

#[test]
fn literal_ifs_bytes_never_delimit() {
    // `x<v>,y` with the expansion `v` producing `a,b`: the literal `,` and
    // `y` are content, so the literal delimiter glues to the field after it —
    // bash: `IFS=,; v="a,b"; printf %s x$v,y` → `[xa][b,y]` (the expansion's
    // own commas cut, the word's literal comma never does).
    assert_eq!(
        split_origin("xa,b,y", &[1, 2, 3], ","),
        vec![b"xa".to_vec(), b"b,y".to_vec()]
    );
}

#[test]
fn expansion_ifs_bytes_delimit_and_literals_glue() {
    // The same text with the delimiter byte inside the span cuts, and the
    // literal tail glues to the field after it.
    assert_eq!(
        split_origin("xa,b", &[1, 2, 3], ","),
        vec![b"xa".to_vec(), b"b".to_vec()]
    );
}

#[test]
fn whitespace_only_expansion_between_literals_splits() {
    // `x$(printf "  ")y`-style: the run is the expansion, both literals are
    // trimmed to fields at the ends of the whole expansion.
    assert_eq!(
        split_origin("x  y", &[1, 2], " \t\n"),
        vec![b"x".to_vec(), b"y".to_vec()]
    );
}

#[test]
fn boundary_starts_a_new_field() {
    // `x` + positional boundary at `b` + `y`: `[x][by]`.
    assert_eq!(
        split_origin_q("xby", &[1, 2], &[1], " "),
        vec![b"x".to_vec(), b"by".to_vec()]
    );
}

#[test]
fn boundary_after_delimiter_flushes_the_pending_empty_field() {
    // `x$@`-style over `a,` + boundary at the (empty) next positional: the
    // delimiter's pending empty field survives (`IFS=,; set -- "a," "";
    // printf %s x$@` → `[xa][]`).
    assert_eq!(
        split_origin_q("xa,y", &[1, 2], &[3], ","),
        vec![b"xa".to_vec(), b"".to_vec(), b"y".to_vec()]
    );
}

#[test]
fn boundary_after_a_consumed_whitespace_run_does_not_flush() {
    // Quoted spelling `IFS=" " set -- " " b; x"$@"y` → `[x ][by]` (bash:
    // `bash -c 'IFS=" "; set -- " " b; printf "[%s]" x"$@"y'` → `[x ][by]`):
    // the protected space stays content, the boundary closes it, and adds
    // no empty field.
    assert_eq!(
        split_origin_quoted("x by", &[2], " "),
        vec![b"x ".to_vec(), b"by".to_vec()]
    );
}

#[test]
fn boundary_at_index_zero_yields_a_leading_empty_field() {
    // `IFS=,; set -- "" b; printf %s $@y` → `[][by]`.
    assert_eq!(
        split_origin_q("by", &[0], &[0], ","),
        vec![b"".to_vec(), b"by".to_vec()]
    );
}

#[test]
fn whitespace_run_stops_at_a_boundary_byte() {
    // Two adjacent expansions each producing one space, glued between
    // literals (`x$(printf " ")$(printf " ")y` → `[x][y]`): the run may not
    // swallow the positional's leading byte, so the boundary at index 2
    // stays visible for the positional that starts with whitespace.
    assert_eq!(
        split_origin_q("x  y", &[1, 2], &[2], " "),
        vec![b"x".to_vec(), b"y".to_vec()]
    );
}
