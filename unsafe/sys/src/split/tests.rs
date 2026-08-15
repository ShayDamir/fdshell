#![allow(clippy::unwrap_used)]
use super::*;

#[test]
fn split_once_mid() {
    let (left, right) = split_once(b"hello=world", b"=").unwrap();
    assert_eq!(left, b"hello");
    assert_eq!(right, b"world");
}

#[test]
fn split_once_start() {
    let (left, right) = split_once(b"=value", b"=").unwrap();
    assert_eq!(left, b"");
    assert_eq!(right, b"value");
}

#[test]
fn split_once_end() {
    let (left, right) = split_once(b"prefix=", b"=").unwrap();
    assert_eq!(left, b"prefix");
    assert_eq!(right, b"");
}

#[test]
fn split_once_none() {
    assert!(split_once(b"hello", b"=").is_none());
}

#[test]
fn split_once_empty() {
    assert!(split_once(b"", b"=").is_none());
}

#[test]
fn split_once_longer_than_data() {
    assert!(split_once(b"ab", b"abc").is_none());
}

#[test]
fn split_once_multibyte_sep() {
    let (left, right) = split_once(b"Umask:\t0022", b"Umask:\t").unwrap();
    assert_eq!(left, b"");
    assert_eq!(right, b"0022");
}

#[test]
fn split_once_repeated_sep() {
    let (left, right) = split_once(b"a=b=c", b"=").unwrap();
    assert_eq!(left, b"a");
    assert_eq!(right, b"b=c");
}
