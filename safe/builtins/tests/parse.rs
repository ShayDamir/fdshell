#![cfg_attr(test, allow(clippy::unwrap_used))]

use core::ffi::CStr;
use std::ffi::CString;
use sys::errno::{EINVAL, ENOENT, HELP};
use sys::fcntl::O_CLOEXEC;

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], code: i32) {
    with_args(args, |a| match builtins::openat2::parse::openat2_parse(a) {
        Err(e) => assert_eq!(e, code),
        _ => panic!("expected Err({code})"),
    });
}

fn assert_ok<F: FnOnce(&builtins::openat2::parse::Openat2Config)>(args: &[&str], f: F) {
    with_args(args, |a| match builtins::openat2::parse::openat2_parse(a) {
        Ok(cfg) => f(&cfg),
        Err(e) => panic!("expected Ok, got Err({e})"),
    });
}

#[test]
fn basic() {
    assert_ok(&["--flags", "O_RDONLY", "package.nix"], |cfg| {
        assert!(cfg.dirfd.is_none());
        assert_eq!(cfg.path.to_bytes(), b"package.nix");
        assert_eq!(cfg.how.flags, 0);
        assert_eq!(cfg.how.mode, 0);
        assert_eq!(cfg.how.resolve, 0);
    });
}

#[test]
fn help_long() {
    assert_err(&["--help"], HELP);
}

#[test]
fn help_short() {
    assert_err(&["-h"], HELP);
}

#[test]
fn empty_args() {
    assert_err(&[], HELP);
}

#[test]
fn dirfd_ateq() {
    assert_ok(&["--dirfd=AT_FDCWD", "x"], |cfg| {
        assert!(cfg.dirfd.is_none());
    });
}

#[test]
fn dirfd_numeric() {
    let (rd, wr) = sys::pipe::pipe2(sys::fcntl::O_CLOEXEC).unwrap();
    rd.verify().unwrap();
    wr.verify().unwrap();
    let dupfd = rd.dup().unwrap();
    let s = format!("{}", dupfd.as_raw());
    assert_ok(&["--dirfd", &s, "x"], |cfg| {
        assert_eq!(cfg.dirfd.as_ref().map(|d| d.as_raw()), Some(dupfd.as_raw()));
    });
    drop(wr);
}

#[test]
fn eq_syntax() {
    assert_ok(&["--flags=O_CLOEXEC", "x"], |cfg| {
        assert_eq!(cfg.how.flags, O_CLOEXEC as u64);
    });
}

#[test]
fn pipe_flags() {
    assert_ok(&["--flags", "O_RDONLY|O_CLOEXEC", "x"], |cfg| {
        assert_eq!(cfg.how.flags, O_CLOEXEC as u64);
    });
}

#[test]
fn resolve_or() {
    assert_ok(
        &["--resolve", "RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS", "x"],
        |cfg| {
            assert_eq!(cfg.how.resolve, 9);
        },
    );
}

#[test]
fn resolve_hex() {
    assert_ok(&["--resolve", "0xff", "x"], |cfg| {
        assert_eq!(cfg.how.resolve, 255);
    });
}

#[test]
fn mode_octal() {
    assert_ok(&["--mode", "644", "x"], |cfg| {
        assert_eq!(cfg.how.mode, 420);
    });
}

#[test]
fn mode_hex() {
    assert_ok(&["--mode", "0x1a4", "x"], |cfg| {
        assert_eq!(cfg.how.mode, 420);
    });
}

#[test]
fn bad_flag() {
    assert_err(&["--bad", "x"], EINVAL);
}

#[test]
fn short_flag() {
    assert_err(&["-f", "O_RDONLY", "x"], EINVAL);
}

#[test]
fn missing_path() {
    assert_err(&["--flags", "O_RDONLY"], EINVAL);
}

#[test]
fn extra_path() {
    assert_err(&["a", "b"], EINVAL);
}

#[test]
fn empty_path() {
    assert_err(&[""], ENOENT);
}

#[test]
fn missing_value() {
    assert_err(&["--flags"], EINVAL);
}

#[test]
fn mode_eq() {
    assert_ok(&["--mode=644", "x"], |cfg| {
        assert_eq!(cfg.how.mode, 420);
    });
}

#[test]
fn resolve_eq() {
    assert_ok(&["--resolve=RESOLVE_IN_ROOT", "x"], |cfg| {
        assert_eq!(cfg.how.resolve, 16);
    });
}
