#![allow(clippy::unwrap_used)]
use super::*;
use crate::shortcstr::ShortCStr;

fn text(bytes: &[u8], line: u32, col: u32) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(line, col),
        Origin::Shell,
    )
}

#[test]
fn position_at_zero_offset() {
    let p = position_at(b"abc", Position::new(2, 5), 0);
    assert_eq!(p, Position::new(2, 5));
}

#[test]
fn position_at_plain_bytes() {
    let p = position_at(b"abcdef", Position::new(1, 1), 3);
    assert_eq!(p, Position::new(1, 4));
}

#[test]
fn position_at_newline_resets_column() {
    let p = position_at(b"ab\ncd", Position::new(1, 10), 4);
    assert_eq!(p, Position::new(2, 2));
}

#[test]
fn position_at_multiple_newlines() {
    let p = position_at(b"a\n\nb", Position::new(5, 1), 4);
    assert_eq!(p, Position::new(7, 2));
}

#[test]
fn position_at_past_end_clamps() {
    let p = position_at(b"ab", Position::new(1, 1), 99);
    assert_eq!(p, Position::new(1, 3));
}

#[test]
fn subslice_inherits_origin_and_advances_start() {
    let t = text(b"echo hi\nmore", 3, 4);
    let sub = t.subslice(8, 4).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), b"more");
    assert_eq!(sub.start, Position::new(4, 1));
    assert_eq!(sub.origin, Origin::Shell);
}

#[test]
fn subslice_empty() {
    let t = text(b"abc", 1, 1);
    let sub = t.subslice(1, 0).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), b"");
    assert_eq!(sub.start, Position::new(1, 2));
}

#[test]
fn subslice_out_of_range_none() {
    let t = text(b"abc", 1, 1);
    assert!(t.subslice(2, 2).is_none());
    assert!(t.subslice(3, 1).is_none());
}
