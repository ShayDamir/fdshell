#![allow(clippy::unwrap_used)]
#![allow(clippy::indexing_slicing)]

use sys::ShortCStr;

#[test]
fn send_recv_roundtrip() {
    let (parent, child) = sys::net::socketpair_with_passcred().unwrap();
    sys::shellfd::set_capture_active(true);
    let pid = sys::env::getpid();
    let word = ShortCStr::from(c"hello world");
    super::send(&child, &word).unwrap();
    let got = super::recv(&parent, pid).unwrap().unwrap();
    assert!(got.eq_bytes(b"hello world"));
}

#[test]
fn send_recv_empty_word() {
    let (parent, child) = sys::net::socketpair_with_passcred().unwrap();
    sys::shellfd::set_capture_active(true);
    let pid = sys::env::getpid();
    super::send(&child, &ShortCStr::new()).unwrap();
    let got = super::recv(&parent, pid).unwrap().unwrap();
    assert!(got.eq_bytes(b""));
}

#[test]
fn recv_eof_is_none() {
    let (parent, child) = sys::net::socketpair_with_passcred().unwrap();
    drop(child);
    let pid = sys::env::getpid();
    let got = super::recv(&parent, pid).unwrap();
    assert!(got.is_none());
}

#[test]
fn is_tag_matches_reserved_tag_only() {
    let reserved: &core::ffi::CStr = c"$_";
    let other: &core::ffi::CStr = c"openat2";
    assert!(super::is_tag(reserved));
    assert!(!super::is_tag(other));
}

#[test]
fn last_word_returns_the_last_field() {
    let fallback = ShortCStr::from(c"cmd");
    let words = [ShortCStr::from(c"one"), ShortCStr::from(c"two")];
    assert!(super::last_word(&words, &fallback).eq_bytes(b"two"));
}

#[test]
fn last_word_falls_back_only_on_an_empty_list() {
    let fallback = ShortCStr::from(c"cmd");
    assert!(super::last_word(&[], &fallback).eq_bytes(b"cmd"));
    // A quoted word that expanded to nothing is an (empty) field: it wins.
    let quoted_empty = [ShortCStr::from(c"one"), ShortCStr::new()];
    assert!(super::last_word(&quoted_empty, &fallback).eq_bytes(b""));
}

#[test]
fn last_word_by_maps_the_last_field() {
    let fallback = ShortCStr::from(c"cmd");
    let map = |w: &ShortCStr| ShortCStr::concat(&[&ShortCStr::from(c"m"), w]);
    let words = [ShortCStr::from(c"one"), ShortCStr::from(c"two")];
    assert!(super::last_word_by(&words, &map, &fallback).eq_bytes(b"mtwo"));
    assert!(super::last_word_by(&[], &map, &fallback).eq_bytes(b"cmd"));
}
