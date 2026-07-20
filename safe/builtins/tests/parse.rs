#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;
use sys::fcntl::{O_APPEND, O_CLOEXEC, O_CREAT, O_EXCL, O_RDWR, O_TRUNC, O_WRONLY};

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], expected: BuiltinError) {
    with_args(args, |a| match builtins::openat2::parse::openat2_parse(a) {
        Err(e) => {
            let ctx = e.current_context();
            match (ctx, expected) {
                (BuiltinError::Help, BuiltinError::Help) => {}
                (BuiltinError::InvalidArgument(_), BuiltinError::InvalidArgument(_)) => {}
                _ => panic!("unexpected error: {ctx}"),
            }
        }
        _ => panic!("expected Err"),
    });
}

fn assert_invalid_arg(args: &[&str]) {
    assert_err(args, BuiltinError::InvalidArgument("x"));
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
    assert_err(&["--help"], BuiltinError::Help);
}

#[test]
fn help_short() {
    assert_err(&["-h"], BuiltinError::Help);
}

#[test]
fn empty_args() {
    assert_err(&[], BuiltinError::Help);
}

#[test]
fn dirfd_ateq() {
    assert_ok(&["--dirfd=AT_FDCWD", "x"], |cfg| {
        assert!(cfg.dirfd.is_none());
    });
}

#[test]
fn dirfd_numeric() {
    let (rd, wr) = sys::pipe::pipe2(0).unwrap();
    rd.verify().unwrap();
    wr.verify().unwrap();
    let dupfd = rd.export().unwrap();
    dupfd.verify().unwrap();
    let s = format!("{}", dupfd.as_raw());
    assert_ok(&["--dirfd", &s, "x"], |cfg| {
        assert_eq!(cfg.dirfd.as_ref().map(|d| d.as_raw()), Some(dupfd.as_raw()));
    });
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
    assert_invalid_arg(&["--bad", "x"])
}

#[test]
fn short_flag() {
    assert_invalid_arg(&["-f", "O_RDONLY", "x"])
}

#[test]
fn missing_path() {
    assert_invalid_arg(&["--flags", "O_RDONLY"])
}

#[test]
fn extra_path() {
    assert_invalid_arg(&["a", "b"])
}

#[test]
fn empty_path() {
    assert_invalid_arg(&[""])
}

#[test]
fn missing_value() {
    assert_invalid_arg(&["--flags"])
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

#[test]
fn pipe_flags_three() {
    assert_ok(&["--flags", "O_CREAT|O_EXCL|O_RDWR", "x"], |cfg| {
        assert_eq!(cfg.how.flags, (O_CREAT | O_EXCL | O_RDWR) as u64);
    });
}

#[test]
fn pipe_flags_write_create_append() {
    assert_ok(&["--flags", "O_WRONLY|O_CREAT|O_APPEND", "x"], |cfg| {
        assert_eq!(cfg.how.flags, (O_WRONLY | O_CREAT | O_APPEND) as u64);
    });
}

#[test]
fn pipe_flags_hex() {
    assert_ok(&["--flags", "0x42", "x"], |cfg| {
        assert_eq!(cfg.how.flags, 0x42u64);
    });
}

#[test]
fn repeated_flags_mixed() {
    assert_ok(
        &["--flags", "O_RDWR|O_CREAT", "--flags", "O_EXCL", "x"],
        |cfg| {
            assert_eq!(cfg.how.flags, (O_RDWR | O_CREAT | O_EXCL) as u64);
        },
    );
}

#[test]
fn pipe_flags_all_names() {
    assert_ok(
        &["--flags", "O_RDWR|O_CREAT|O_EXCL|O_TRUNC|O_APPEND", "x"],
        |cfg| {
            assert_eq!(
                cfg.how.flags,
                (O_RDWR | O_CREAT | O_EXCL | O_TRUNC | O_APPEND) as u64
            );
        },
    );
}

#[test]
fn repeated_flags_individual() {
    assert_ok(
        &[
            "--flags", "O_RDWR", "--flags", "O_CREAT", "--flags", "O_EXCL", "x",
        ],
        |cfg| {
            assert_eq!(cfg.how.flags, (O_RDWR | O_CREAT | O_EXCL) as u64);
        },
    );
}
