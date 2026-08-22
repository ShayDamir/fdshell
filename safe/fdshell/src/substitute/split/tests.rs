#![allow(clippy::unwrap_used)]

use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

use super::split_word;

fn split(word: &str, ifs: &str) -> Vec<Vec<u8>> {
    split_word(
        &ShortCStr::from_vec(word.as_bytes().to_vec()).unwrap(),
        &ShortCStr::from_vec(ifs.as_bytes().to_vec()).unwrap(),
    )
    .unwrap()
    .iter()
    .map(|s| s.as_bytes().unwrap().to_vec())
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
fn non_whitespace_ifs_keeps_trailing_empty_field() {
    assert_eq!(split("a:", ":"), vec![b"a".to_vec(), b"".to_vec()]);
}

#[test]
fn non_whitespace_ifs_keeps_leading_empty_field() {
    assert_eq!(split(":a", ":"), vec![b"".to_vec(), b"a".to_vec()]);
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
