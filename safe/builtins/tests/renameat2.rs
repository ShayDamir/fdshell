#![cfg_attr(test, allow(clippy::unwrap_used))]

use core::ffi::CStr;
use std::ffi::CString;
use sys::errno::{EINVAL, ENOENT, HELP};
use sys::renameat2::{RENAME_EXCHANGE, RENAME_NOREPLACE};

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], code: i32) {
    with_args(
        args,
        |a| match builtins::renameat2::parse::renameat2_parse(a) {
            Err(e) => assert_eq!(e, code),
            _ => panic!("expected Err({code})"),
        },
    );
}

fn assert_ok<F: FnOnce(&builtins::renameat2::parse::Renameat2Config)>(args: &[&str], f: F) {
    with_args(
        args,
        |a| match builtins::renameat2::parse::renameat2_parse(a) {
            Ok(cfg) => f(&cfg),
            Err(e) => panic!("expected Ok, got Err({e})"),
        },
    );
}

#[test]
fn basic() {
    assert_ok(&["old", "new"], |cfg| {
        assert!(cfg.olddirfd.is_none());
        assert!(cfg.newdirfd.is_none());
        assert_eq!(cfg.oldpath.to_bytes(), b"old");
        assert_eq!(cfg.newpath.to_bytes(), b"new");
        assert_eq!(cfg.flags, 0);
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
fn bad_flag() {
    assert_err(&["--bad", "x", "y"], EINVAL);
}

#[test]
fn missing_oldpath() {
    assert_err(&["--flags", "RENAME_NOREPLACE"], EINVAL);
}

#[test]
fn missing_newpath() {
    assert_err(&["old"], EINVAL);
}

#[test]
fn extra_path() {
    assert_err(&["a", "b", "c"], EINVAL);
}

#[test]
fn empty_oldpath() {
    assert_err(&["", "new"], ENOENT);
}

#[test]
fn empty_newpath() {
    assert_err(&["old", ""], ENOENT);
}

#[test]
fn missing_value() {
    assert_err(&["--flags", "--olddirfd", "5", "a", "b"], EINVAL);
}

#[test]
fn olddirfd_ateq() {
    assert_ok(&["--olddirfd=AT_FDCWD", "old", "new"], |cfg| {
        assert!(cfg.olddirfd.is_none());
    });
}

#[test]
fn olddirfd_numeric() {
    let (rd, wr) = sys::pipe::pipe2(sys::fcntl::O_CLOEXEC).unwrap();
    rd.verify().unwrap();
    wr.verify().unwrap();
    let dupfd = rd.export().unwrap();
    let s = format!("{}", dupfd.as_raw());
    assert_ok(&["--olddirfd", &s, "old", "new"], |cfg| {
        assert_eq!(
            cfg.olddirfd.as_ref().map(|d| d.as_raw()),
            Some(dupfd.as_raw())
        );
    });
}

#[test]
fn newdirfd_ateq() {
    assert_ok(&["--newdirfd=AT_FDCWD", "old", "new"], |cfg| {
        assert!(cfg.newdirfd.is_none());
    });
}

#[test]
fn newdirfd_numeric() {
    let (rd, wr) = sys::pipe::pipe2(sys::fcntl::O_CLOEXEC).unwrap();
    rd.verify().unwrap();
    wr.verify().unwrap();
    let dupfd = rd.export().unwrap();
    let s = format!("{}", dupfd.as_raw());
    assert_ok(&["--newdirfd", &s, "old", "new"], |cfg| {
        assert_eq!(
            cfg.newdirfd.as_ref().map(|d| d.as_raw()),
            Some(dupfd.as_raw())
        );
    });
}

#[test]
fn flags_noreplace() {
    assert_ok(&["--flags", "RENAME_NOREPLACE", "old", "new"], |cfg| {
        assert_eq!(cfg.flags, RENAME_NOREPLACE);
    });
}

#[test]
fn flags_exchange() {
    assert_ok(&["--flags", "RENAME_EXCHANGE", "old", "new"], |cfg| {
        assert_eq!(cfg.flags, RENAME_EXCHANGE);
    });
}

#[test]
fn flags_noreplace_exchange() {
    assert_ok(
        &["--flags", "RENAME_NOREPLACE|RENAME_EXCHANGE", "old", "new"],
        |cfg| {
            assert_eq!(cfg.flags, RENAME_NOREPLACE | RENAME_EXCHANGE);
        },
    );
}

#[test]
fn flags_hex() {
    assert_ok(&["--flags", "0x3", "old", "new"], |cfg| {
        assert_eq!(cfg.flags, RENAME_NOREPLACE | RENAME_EXCHANGE);
    });
}

#[test]
fn eq_syntax() {
    assert_ok(&["--flags=RENAME_EXCHANGE", "old", "new"], |cfg| {
        assert_eq!(cfg.flags, RENAME_EXCHANGE);
    });
}

#[test]
fn test_renameat2_exec() {
    let dir = std::env::temp_dir().join("fdshell-test-renameat2-exec");
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();

    let old_path = dir.join("old.txt");
    let new_path = dir.join("new.txt");
    std::fs::write(&old_path, "hello").unwrap();

    let old_cs = CString::new(old_path.to_str().unwrap()).unwrap();
    let new_cs = CString::new(new_path.to_str().unwrap()).unwrap();
    let args = [old_cs.as_c_str(), new_cs.as_c_str()];
    let cfg = builtins::renameat2::parse::renameat2_parse(&args).unwrap();
    builtins::renameat2::renameat2_exec(&cfg).unwrap();

    assert!(!old_path.exists(), "old path should not exist after rename");
    assert!(new_path.exists(), "new path should exist after rename");
    assert_eq!(std::fs::read_to_string(&new_path).unwrap(), "hello");

    std::fs::remove_dir_all(&dir).unwrap();
}
