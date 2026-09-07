#![allow(clippy::unwrap_used)]

use builtins::error::BuiltinError;
use sys::ShortCStr;

use super::dir_var;
use super::split_eq;

fn scs(s: &str) -> ShortCStr {
    ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap()
}

fn is_dir_var_err(e: &error_stack::Report<BuiltinError>) -> bool {
    matches!(
        e.current_context(),
        BuiltinError::InvalidArgument(what) if *what == "dir var"
    )
}

#[test]
fn split_eq_both_forms() {
    let attached = scs("--dir=%d");
    assert_eq!(
        split_eq(&attached).unwrap(),
        (&b"--dir"[..], Some(&b"%d"[..]))
    );
    let bare = scs("--nofollow");
    assert_eq!(split_eq(&bare).unwrap(), (&b"--nofollow"[..], None));
}

#[test]
fn split_eq_empty_value() {
    let attached = scs("--dir=");
    assert_eq!(
        split_eq(&attached).unwrap(),
        (&b"--dir"[..], Some(&b""[..]))
    );
}

#[test]
fn dir_var_strips_the_percent() {
    let name = dir_var(b"%d").unwrap();
    assert_eq!(name.as_bytes().unwrap(), b"d");
    let name = dir_var(b"%a_b").unwrap();
    assert_eq!(name.as_bytes().unwrap(), b"a_b");
}

#[test]
fn dir_var_rejects_bad_forms() {
    for v in [b"d" as &[u8], b"%" as &[u8], b"%d%e" as &[u8], b"" as &[u8]] {
        let e = dir_var(v).unwrap_err();
        assert!(is_dir_var_err(&e), "{v:?}");
    }
}
