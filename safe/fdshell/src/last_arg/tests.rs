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
