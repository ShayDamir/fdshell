#![allow(clippy::indexing_slicing, clippy::unwrap_used)]

use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

use super::list::join;
use super::split::Parts;

fn parts(w: &[u8]) -> Parts {
    super::split::split(w, &[])
}

fn comp_names(p: &Parts) -> Vec<&[u8]> {
    p.comps.iter().map(|c| c.pat.as_slice()).collect()
}

#[test]
fn simple_relative_components() {
    let p = parts(b"a/b/c");
    assert!(!p.absolute);
    assert!(!p.dir_only);
    assert_eq!(
        comp_names(&p),
        vec![b"a".as_slice(), b"b".as_slice(), b"c".as_slice()]
    );
}

#[test]
fn leading_slash_marks_absolute_and_drops_empty_component() {
    let p = parts(b"/a/b");
    assert!(p.absolute);
    assert!(!p.dir_only);
    assert_eq!(comp_names(&p), vec![b"a".as_slice(), b"b".as_slice()]);
}

#[test]
fn trailing_slash_marks_dir_only() {
    let p = parts(b"a/");
    assert!(!p.absolute);
    assert!(p.dir_only);
    assert_eq!(comp_names(&p), vec![b"a".as_slice()]);
}

#[test]
fn double_slash_collapses_empty_component() {
    let p = parts(b"a//b");
    assert!(!p.absolute);
    assert!(!p.dir_only);
    assert_eq!(comp_names(&p), vec![b"a".as_slice(), b"b".as_slice()]);
}

#[test]
fn quoted_slash_does_not_split() {
    // `a/"b/c"`: the second slash is inside quotes and stays in the name.
    let p = super::split::split(b"a/b/c", &[false, false, true, true, true]);
    assert_eq!(comp_names(&p), vec![b"a".as_slice(), b"b/c".as_slice()]);
    assert!(!p.dir_only);
}

#[test]
fn escaped_slash_does_not_split() {
    let p = parts(b"a\\/b");
    assert_eq!(comp_names(&p), vec![b"a\\/b".as_slice()]);
}

#[test]
fn bracket_span_with_slash_does_not_split() {
    let p = parts(b"a/[x/y]");
    assert_eq!(comp_names(&p), vec![b"a".as_slice(), b"[x/y]".as_slice()]);
}

#[test]
fn literal_and_pattern_components_flagged() {
    let p = parts(b"sub/*");
    assert!(p.comps[0].literal);
    assert!(!p.comps[1].literal);
}

#[test]
fn unclosed_bracket_is_literal() {
    let p = parts(b"[abc");
    assert!(p.comps[0].literal);
    assert_eq!(comp_names(&p), vec![b"[abc".as_slice()]);
}

#[test]
fn valid_bracket_is_pattern() {
    let p = parts(b"[ab]1");
    assert!(!p.comps[0].literal);
}

#[test]
fn star_with_trailing_slash_is_dir_only_pattern() {
    let p = parts(b"*/");
    assert!(p.dir_only);
    assert!(!p.comps[0].literal);
}

#[test]
fn join_appends_name_and_slash() {
    // Prefixes in the walk always end in `/` (or are empty for the root).
    let prefix = ShortCStr::from(c"/a/");
    let name = ShortCStr::from(c"b");
    let j = join(&prefix, &name).unwrap();
    assert_eq!(j.as_bytes().unwrap(), b"/a/b/");
}

#[test]
fn push_result_appends_trailing_slash_when_directories_only() {
    let mut out = Vec::new();
    let prefix = ShortCStr::from(c"");
    let name = ShortCStr::from(c"sub");
    super::list::push_result(&mut out, &prefix, &name, true);
    assert_eq!(out, vec![b"sub/".to_vec()]);
}
