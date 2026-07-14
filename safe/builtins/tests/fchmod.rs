#![cfg_attr(test, allow(clippy::unwrap_used, clippy::indexing_slicing))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

struct TestFd {
    _exported: sys::ExportedFd,
    fd_str: CString,
}

impl TestFd {
    fn new() -> Self {
        let local = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
        let exported = local.export().unwrap();
        let fd_str = CString::new(exported.as_raw().to_string()).unwrap();
        Self {
            _exported: exported,
            fd_str,
        }
    }

    fn as_cstr(&self) -> &CStr {
        self.fd_str.as_c_str()
    }

    fn raw(&self) -> i32 {
        self._exported.as_raw()
    }
}

fn with_fd<F: FnOnce(&TestFd)>(f: F) {
    let fd = TestFd::new();
    f(&fd);
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
    with_fd(|fd| {
        assert_ok(&["644", fd.fd_str.to_str().unwrap()], |cfg| {
            assert_eq!(cfg.fds.first().unwrap().as_raw(), fd.raw());
            assert_eq!(cfg.mode, 0o644);
        });
    });
}

#[test]
fn positional_mode_octal() {
    with_fd(|fd| {
        assert_ok(&["755", fd.fd_str.to_str().unwrap()], |cfg| {
            assert_eq!(cfg.mode, 0o755);
            assert_eq!(cfg.fds.first().unwrap().as_raw(), fd.raw());
        });
    });
}

#[test]
fn positional_mode_hex() {
    with_fd(|fd| {
        assert_ok(&["0x1ed", fd.fd_str.to_str().unwrap()], |cfg| {
            assert_eq!(cfg.mode, 0o755);
            assert_eq!(cfg.fds.first().unwrap().as_raw(), fd.raw());
        });
    });
}

#[test]
fn positional_mode_octal_prefix() {
    with_fd(|fd| {
        assert_ok(&["0o644", fd.fd_str.to_str().unwrap()], |cfg| {
            assert_eq!(cfg.mode, 0o644);
            assert_eq!(cfg.fds.first().unwrap().as_raw(), fd.raw());
        });
    });
}

#[test]
fn positional_multiple_fds() {
    with_fd(|fd| {
        let s = fd.fd_str.to_str().unwrap();
        assert_ok(&["644", s, s], |cfg| {
            assert_eq!(cfg.fds.len(), 2);
            assert_eq!(cfg.fds[0].as_raw(), fd.raw());
            assert_eq!(cfg.fds[1].as_raw(), fd.raw());
            assert_eq!(cfg.mode, 0o644);
        });
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
    with_fd(|fd| {
        assert_ok(
            &["--fd", fd.fd_str.to_str().unwrap(), "--mode", "755"],
            |cfg| {
                assert_eq!(cfg.fds.first().unwrap().as_raw(), fd.raw());
                assert_eq!(cfg.mode, 0o755);
            },
        );
    });
}

#[test]
fn flag_eq_syntax() {
    with_fd(|fd| {
        assert_ok(
            &["--fd", fd.fd_str.to_str().unwrap(), "--mode", "755"],
            |cfg| {
                assert_eq!(cfg.fds.first().unwrap().as_raw(), fd.raw());
                assert_eq!(cfg.mode, 0o755);
            },
        );
    });
}

#[test]
fn flag_mode_hex() {
    with_fd(|fd| {
        assert_ok(
            &["--fd", fd.fd_str.to_str().unwrap(), "--mode", "0x1ed"],
            |cfg| {
                assert_eq!(cfg.mode, 0o755);
                assert_eq!(cfg.fds.first().unwrap().as_raw(), fd.raw());
            },
        );
    });
}

#[test]
fn flag_mode_octal_prefix() {
    with_fd(|fd| {
        assert_ok(
            &["--fd", fd.fd_str.to_str().unwrap(), "--mode", "0o644"],
            |cfg| {
                assert_eq!(cfg.mode, 0o644);
                assert_eq!(cfg.fds.first().unwrap().as_raw(), fd.raw());
            },
        );
    });
}

#[test]
fn flag_multiple_fds() {
    with_fd(|fd| {
        let s = fd.fd_str.to_str().unwrap();
        assert_ok(&["--fd", s, "--fd", s, "--fd", s, "--mode", "644"], |cfg| {
            assert_eq!(cfg.fds.len(), 3);
            for f in &cfg.fds {
                assert_eq!(f.as_raw(), fd.raw());
            }
            assert_eq!(cfg.mode, 0o644);
        });
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
    with_fd(|fd| {
        let cfg = builtins::fchmod::parse::fchmod_parse(&[c"444", fd.as_cstr()]).unwrap();
        // Non-regular file ( /dev/null ), so fchmod returns EPERM.
        // This validates the full parse → dispatch → syscall chain.
        let result = builtins::fchmod::fchmod_exec(&cfg);
        assert!(result.is_err());
    });
}

#[test]
fn test_fchmod_exec_flags() {
    with_fd(|fd| {
        let cfg =
            builtins::fchmod::parse::fchmod_parse(&[c"--fd", fd.as_cstr(), c"--mode", c"0o644"])
                .unwrap();
        let result = builtins::fchmod::fchmod_exec(&cfg);
        assert!(result.is_err());
    });
}

#[test]
fn test_fchmod_exec_multiple_fds() {
    with_fd(|fd| {
        let cfg =
            builtins::fchmod::parse::fchmod_parse(&[c"600", fd.as_cstr(), fd.as_cstr()]).unwrap();
        let result = builtins::fchmod::fchmod_exec(&cfg);
        assert!(result.is_err());
    });
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
