#![allow(clippy::unwrap_used)]

use super::*;

fn drain_bytes(data: &[u8], limit: usize) -> Result<Vec<u8>, Report<CmdSubstError>> {
    let (r, w) = sys::pipe::pipe2(0).unwrap();
    sys::rw::write(&w, data).unwrap();
    drop(w);
    drain(&r, limit)
}

#[test]
fn drain_strips_trailing_newlines() {
    let out = drain_bytes(b"hello\n\n", 1024).unwrap();
    assert_eq!(out, b"hello");
}

#[test]
fn drain_under_cap_returns_all() {
    let out = drain_bytes(b"abc", 3).unwrap();
    assert_eq!(out, b"abc");
}

#[test]
fn drain_over_cap_fails() {
    let result = drain_bytes(b"abcdefgh", 3);
    assert!(matches!(
        result.unwrap_err().current_context(),
        CmdSubstError::OutputTooLarge
    ));
}

#[test]
fn drain_empty_is_empty() {
    let out = drain_bytes(b"", 1024).unwrap();
    assert!(out.is_empty());
}
