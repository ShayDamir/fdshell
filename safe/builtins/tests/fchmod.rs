#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], expected: BuiltinError) {
    with_args(args, |a| match builtins::fchmod::parse::fchmod_parse(a) {
        Err(e) => {
            let ctx = e.current_context();
            match (ctx, expected) {
                (BuiltinError::Help, BuiltinError::Help) => {}
                (BuiltinError::InvalidArgument(_), BuiltinError::InvalidArgument(_)) => {}
                (BuiltinError::MissingArgument(_), BuiltinError::MissingArgument(_)) => {}
                (BuiltinError::InvalidArgument(_), BuiltinError::MissingArgument(_)) => {}
                (BuiltinError::MissingArgument(_), BuiltinError::InvalidArgument(_)) => {}
                _ => panic!("unexpected error: {ctx}"),
            }
        }
        Ok(cfg) => panic!(
            "expected Err, got Ok with fds={:?} mode={:o}",
            cfg.fds.iter().map(|f| f.as_raw()).collect::<Vec<_>>(),
            cfg.mode
        ),
    });
}

fn assert_invalid_arg(args: &[&str]) {
    assert_err(args, BuiltinError::InvalidArgument("x"));
}

fn assert_ok<F: FnOnce(&builtins::fchmod::parse::FchmodConfig)>(args: &[&str], f: F) {
    with_args(args, |a| match builtins::fchmod::parse::fchmod_parse(a) {
        Ok(cfg) => f(&cfg),
        Err(e) => panic!("expected Ok, got Err({e})"),
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
fn bad_flag() {
    assert_invalid_arg(&["--bad", "3"])
}

#[test]
fn missing_fd_flag() {
    assert_invalid_arg(&["--mode", "755"])
}

#[test]
fn missing_mode_flag() {
    assert_invalid_arg(&["--fd", "3"])
}

#[test]
fn missing_value() {
    assert_invalid_arg(&["--fd"])
}

#[test]
fn invalid_flag_value() {
    assert_invalid_arg(&["--mode"])
}

#[test]
fn unknown_flag() {
    assert_invalid_arg(&["--fd", "3", "--xyz"])
}

#[test]
fn positional_basic() {
    assert_ok(&["644", "0"], |cfg| {
        assert_eq!(cfg.fds.first().unwrap().as_raw(), 0);
        assert_eq!(cfg.mode, 0o644);
    });
}

#[test]
fn positional_multiple_fds() {
    assert_ok(&["644", "0", "1"], |cfg| {
        let fds: Vec<i32> = cfg.fds.iter().map(|f| f.as_raw()).collect();
        assert!(fds.contains(&0));
        assert!(fds.contains(&1));
        assert_eq!(cfg.mode, 0o644);
    });
}

#[test]
fn positional_mode_octal() {
    assert_ok(&["755", "0"], |cfg| {
        assert_eq!(cfg.mode, 0o755);
        assert_eq!(cfg.fds.first().unwrap().as_raw(), 0);
    });
}

#[test]
fn positional_mode_hex() {
    assert_ok(&["0x1ed", "0"], |cfg| {
        assert_eq!(cfg.mode, 0o755);
        assert_eq!(cfg.fds.first().unwrap().as_raw(), 0);
    });
}

#[test]
fn positional_mode_octal_prefix() {
    assert_ok(&["0o644", "0"], |cfg| {
        assert_eq!(cfg.mode, 0o644);
        assert_eq!(cfg.fds.first().unwrap().as_raw(), 0);
    });
}

#[test]
fn positional_too_few() {
    assert_err(&["644"], BuiltinError::MissingArgument("fd"));
}

#[test]
fn positional_negative_fd() {
    assert_invalid_arg(&["644", "-1"])
}

#[test]
fn flag_basic() {
    assert_ok(&["--fd", "0", "--mode", "755"], |cfg| {
        assert_eq!(cfg.fds.first().unwrap().as_raw(), 0);
        assert_eq!(cfg.mode, 0o755);
    });
}

#[test]
fn flag_multiple_fds() {
    assert_ok(
        &["--fd", "0", "--fd", "1", "--fd", "2", "--mode", "644"],
        |cfg| {
            let fds: Vec<i32> = cfg.fds.iter().map(|f| f.as_raw()).collect();
            assert!(fds.contains(&0));
            assert!(fds.contains(&1));
            assert!(fds.contains(&2));
            assert_eq!(cfg.mode, 0o644);
        },
    );
}

#[test]
fn flag_eq_syntax() {
    assert_ok(&["--fd=0", "--mode=755"], |cfg| {
        assert_eq!(cfg.fds.first().unwrap().as_raw(), 0);
        assert_eq!(cfg.mode, 0o755);
    });
}

#[test]
fn flag_mode_hex() {
    assert_ok(&["--fd", "0", "--mode", "0x1ed"], |cfg| {
        assert_eq!(cfg.mode, 0o755);
        assert_eq!(cfg.fds.first().unwrap().as_raw(), 0);
    });
}

#[test]
fn flag_mode_octal_prefix() {
    assert_ok(&["--fd", "0", "--mode", "0o644"], |cfg| {
        assert_eq!(cfg.mode, 0o644);
        assert_eq!(cfg.fds.first().unwrap().as_raw(), 0);
    });
}

#[test]
fn flag_invalid_fd() {
    assert_invalid_arg(&["--fd", "-1", "--mode", "755"]);
}

#[test]
fn flag_non_numeric_fd() {
    assert_invalid_arg(&["--fd", "abc", "--mode", "755"]);
}

#[test]
fn test_fchmod_exec() {
    let cfg = builtins::fchmod::parse::fchmod_parse(&[c"444", c"0"]).unwrap();
    // stdin (fd 0) is not a regular file, so fchmod returns EPERM.
    // This validates the full parse → dispatch → syscall chain.
    let result = builtins::fchmod::fchmod_exec(&cfg);
    assert!(result.is_err());
}

#[test]
fn test_fchmod_exec_multiple_fds() {
    let cfg = builtins::fchmod::parse::fchmod_parse(&[c"600", c"0", c"1"]).unwrap();
    let result = builtins::fchmod::fchmod_exec(&cfg);
    assert!(result.is_err());
}

#[test]
fn test_fchmod_exec_flags() {
    let cfg = builtins::fchmod::parse::fchmod_parse(&[c"--fd", c"0", c"--mode", c"0o644"]).unwrap();
    let result = builtins::fchmod::fchmod_exec(&cfg);
    assert!(result.is_err());
}

#[test]
fn test_fchmod_ebadf() {
    // openat2 returns a CLOEXEC fd; ImportedFd::from_bytes rejects it via verify().
    let cloexec_fd = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
    let fd_str = format!("{}", cloexec_fd.as_raw());
    let result =
        builtins::fchmod::parse::fchmod_parse(&[c"644", CString::new(fd_str).unwrap().as_c_str()]);
    assert!(result.is_err());
}

#[test]
fn test_fchmod_exec_ebadf() {
    // Non-existent fd number — ImportedFd::from_bytes fails because
    // fcntl(F_GETFD) on an invalid fd returns EBADF.
    let result = builtins::fchmod::parse::fchmod_parse(&[c"644", c"99999"]);
    assert!(result.is_err());
}
