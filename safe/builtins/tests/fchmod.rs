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
                _ => panic!("unexpected error: {ctx}"),
            }
        }
        Ok(cfg) => panic!(
            "expected Err, got Ok with fds={:?} mode={:o}",
            cfg.fds, cfg.mode
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
    assert_ok(&["644", "3"], |cfg| {
        assert_eq!(cfg.fds, [3]);
        assert_eq!(cfg.mode, 0o644);
    });
}

#[test]
fn positional_multiple_fds() {
    assert_ok(&["644", "3", "5", "7"], |cfg| {
        assert_eq!(cfg.fds, [3, 5, 7]);
        assert_eq!(cfg.mode, 0o644);
    });
}

#[test]
fn positional_mode_octal() {
    assert_ok(&["755", "3"], |cfg| {
        assert_eq!(cfg.mode, 0o755);
        assert_eq!(cfg.fds, [3]);
    });
}

#[test]
fn positional_mode_hex() {
    assert_ok(&["0x1ed", "3"], |cfg| {
        assert_eq!(cfg.mode, 0o755);
    });
}

#[test]
fn positional_mode_octal_prefix() {
    assert_ok(&["0o644", "3"], |cfg| {
        assert_eq!(cfg.mode, 0o644);
    });
}

#[test]
fn positional_too_few() {
    assert_invalid_arg(&["644"])
}

#[test]
fn positional_negative_fd() {
    assert_invalid_arg(&["644", "-1"])
}

#[test]
fn flag_basic() {
    assert_ok(&["--fd", "3", "--mode", "755"], |cfg| {
        assert_eq!(cfg.fds, [3]);
        assert_eq!(cfg.mode, 0o755);
    });
}

#[test]
fn flag_multiple_fds() {
    assert_ok(
        &["--fd", "3", "--fd", "5", "--fd", "7", "--mode", "644"],
        |cfg| {
            assert_eq!(cfg.fds, [3, 5, 7]);
            assert_eq!(cfg.mode, 0o644);
        },
    );
}

#[test]
fn flag_eq_syntax() {
    assert_ok(&["--fd=3", "--mode=755"], |cfg| {
        assert_eq!(cfg.fds, [3]);
        assert_eq!(cfg.mode, 0o755);
    });
}

#[test]
fn flag_mode_hex() {
    assert_ok(&["--fd", "3", "--mode", "0x1ed"], |cfg| {
        assert_eq!(cfg.mode, 0o755);
    });
}

#[test]
fn flag_mode_octal_prefix() {
    assert_ok(&["--fd", "3", "--mode", "0o644"], |cfg| {
        assert_eq!(cfg.mode, 0o644);
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
    let dir = std::env::temp_dir().join("fdshell-test-fchmod-exec");
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("test.txt");
    std::fs::write(&file_path, b"hello").unwrap();

    let file = std::fs::File::open(&file_path).unwrap();
    let raw = std::os::unix::io::IntoRawFd::into_raw_fd(file);
    // SAFETY: `raw` was just obtained from `IntoRawFd::into_raw_fd()` on a valid `File`,
    // guaranteeing it refers to an open fd with no other owner.
    let fd = unsafe { sys::LocalFd::from_raw(raw) };

    let fd_str = format!("{}", fd.as_raw());
    let cfg = builtins::fchmod::parse::fchmod_parse(&[
        CString::new("444").unwrap().as_c_str(),
        CString::new(fd_str).unwrap().as_c_str(),
    ])
    .unwrap();
    builtins::fchmod::fchmod_exec(&cfg).unwrap();

    let st = sys::stat::stat(&CString::new(file_path.to_str().unwrap()).unwrap()).unwrap();
    assert_eq!(st.mode & 0o777, 0o444, "expected mode 0444");

    fd.try_close().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_fchmod_exec_multiple_fds() {
    let dir = std::env::temp_dir().join("fdshell-test-fchmod-multi");
    std::fs::create_dir_all(&dir).unwrap();

    let file_a = dir.join("a.txt");
    let file_b = dir.join("b.txt");
    std::fs::write(&file_a, b"a").unwrap();
    std::fs::write(&file_b, b"b").unwrap();

    // SAFETY: `into_raw_fd` consumes a valid `File`, yielding an open fd with no other owner.
    let fd_a = unsafe {
        sys::LocalFd::from_raw(std::os::unix::io::IntoRawFd::into_raw_fd(
            std::fs::File::open(&file_a).unwrap(),
        ))
    };
    // SAFETY: ditto.
    let fd_b = unsafe {
        sys::LocalFd::from_raw(std::os::unix::io::IntoRawFd::into_raw_fd(
            std::fs::File::open(&file_b).unwrap(),
        ))
    };

    let s_a = format!("{}", fd_a.as_raw());
    let s_b = format!("{}", fd_b.as_raw());

    let cfg = builtins::fchmod::parse::fchmod_parse(&[
        CString::new("600").unwrap().as_c_str(),
        CString::new(s_a).unwrap().as_c_str(),
        CString::new(s_b).unwrap().as_c_str(),
    ])
    .unwrap();
    builtins::fchmod::fchmod_exec(&cfg).unwrap();

    let path = CString::new(file_a.to_str().unwrap()).unwrap();
    let st = sys::stat::stat(&path).unwrap();
    assert_eq!(st.mode & 0o777, 0o600);

    let path = CString::new(file_b.to_str().unwrap()).unwrap();
    let st = sys::stat::stat(&path).unwrap();
    assert_eq!(st.mode & 0o777, 0o600);

    fd_a.try_close().unwrap();
    fd_b.try_close().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_fchmod_exec_flags() {
    let dir = std::env::temp_dir().join("fdshell-test-fchmod-flags");
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("f.txt");
    std::fs::write(&file_path, b"data").unwrap();

    // SAFETY: `into_raw_fd` consumes a valid `File`, yielding an open fd with no other owner.
    let fd = unsafe {
        sys::LocalFd::from_raw(std::os::unix::io::IntoRawFd::into_raw_fd(
            std::fs::File::open(&file_path).unwrap(),
        ))
    };

    let s = format!("{}", fd.as_raw());

    let cfg = builtins::fchmod::parse::fchmod_parse(&[
        CString::new("--fd").unwrap().as_c_str(),
        CString::new(s).unwrap().as_c_str(),
        CString::new("--mode").unwrap().as_c_str(),
        CString::new("0o644").unwrap().as_c_str(),
    ])
    .unwrap();
    builtins::fchmod::fchmod_exec(&cfg).unwrap();

    let path = CString::new(file_path.to_str().unwrap()).unwrap();
    let st = sys::stat::stat(&path).unwrap();
    assert_eq!(st.mode & 0o777, 0o644);

    fd.try_close().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_fchmod_ebadf() {
    // SAFETY: -1 is never a valid fd; `fchmod` returns `EBADF`.
    let fd = unsafe { sys::LocalFd::from_raw(-1) };
    assert_eq!(
        sys::fchmod::fchmod(fd.as_raw(), 0o644),
        Err(sys::SyscallError::EBADF)
    );
    // fd intentionally not closed (it was never valid).
}
