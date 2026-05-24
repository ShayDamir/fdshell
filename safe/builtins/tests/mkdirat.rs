#![cfg_attr(test, allow(clippy::unwrap_used))]

use core::ffi::CStr;
use std::ffi::CString;
use sys::shellfd::TAG_MAX;

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], code: i32) {
    with_args(args, |a| match builtins::mkdirat::parse::mkdirat_parse(a) {
        Err(e) => assert_eq!(e, code),
        _ => panic!("expected Err({code})"),
    });
}

fn assert_ok<F: FnOnce(&builtins::mkdirat::parse::MkdiratConfig)>(args: &[&str], f: F) {
    with_args(args, |a| match builtins::mkdirat::parse::mkdirat_parse(a) {
        Ok(cfg) => f(&cfg),
        Err(e) => panic!("expected Ok, got Err({e})"),
    });
}

#[test]
fn basic() {
    assert_ok(&["--mode", "755", "newdir"], |cfg| {
        assert!(cfg.dirfd.is_none());
        assert_eq!(cfg.mode, 0o755);
        assert_eq!(cfg.path.to_bytes(), b"newdir");
    });
}

#[test]
fn help_long() {
    assert_err(&["--help"], 0);
}

#[test]
fn help_short() {
    assert_err(&["-h"], 0);
}

#[test]
fn empty_args() {
    assert_err(&[], 0);
}

#[test]
fn bad_flag() {
    assert_err(&["--bad", "x"], 22);
}

#[test]
fn missing_path() {
    assert_err(&["--mode", "755"], 22);
}

#[test]
fn extra_path() {
    assert_err(&["a", "b"], 22);
}

#[test]
fn empty_path() {
    assert_err(&[""], 2);
}

#[test]
fn missing_value() {
    assert_err(&["--mode"], 22);
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
    let _fd5 = rd.dup3(5).unwrap();
    assert_ok(&["--dirfd", "5", "x"], |cfg| {
        assert_eq!(cfg.dirfd.as_ref().map(|d| d.as_raw()), Some(5));
    });
    drop(wr);
}

#[test]
fn mode_octal() {
    assert_ok(&["--mode", "755", "x"], |cfg| {
        assert_eq!(cfg.mode, 0o755);
    });
}

#[test]
fn mode_hex() {
    assert_ok(&["--mode", "0x1ed", "x"], |cfg| {
        assert_eq!(cfg.mode, 0o755);
    });
}

#[test]
fn mode_octal_prefix() {
    assert_ok(&["--mode", "0o755", "x"], |cfg| {
        assert_eq!(cfg.mode, 0o755);
    });
}

#[test]
fn resolve_single() {
    assert_ok(&["--resolve", "RESOLVE_BENEATH", "x"], |cfg| {
        assert_eq!(cfg.resolve, 8);
    });
}

#[test]
fn resolve_or() {
    assert_ok(
        &["--resolve", "RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS", "x"],
        |cfg| {
            assert_eq!(cfg.resolve, 9);
        },
    );
}

#[test]
fn resolve_hex() {
    assert_ok(&["--resolve", "0xff", "x"], |cfg| {
        assert_eq!(cfg.resolve, 255);
    });
}

#[test]
fn eq_syntax() {
    assert_ok(&["--mode=755", "x"], |cfg| {
        assert_eq!(cfg.mode, 0o755);
    });
}

#[test]
fn test_mkdirat_exec() {
    let dir = std::env::temp_dir().join("fdshell-test-mkdirat-exec");
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let subdir_path = dir.join("subdir");

    let cpath = CString::new(subdir_path.to_str().unwrap()).unwrap();

    sys::shellfd::reserve_shellfd().unwrap();
    let (a, b) = sys::net::socketpair().unwrap();
    a.dup2(sys::shellfd::SHELL_DUPFD).unwrap();
    a.close().unwrap();
    let receiver = b;

    let mode_arg = CString::new("--mode").unwrap();
    let mode_val = CString::new("755").unwrap();
    let args = [mode_arg.as_c_str(), mode_val.as_c_str(), cpath.as_c_str()];
    let cfg = builtins::mkdirat::parse::mkdirat_parse(&args).unwrap();
    builtins::mkdirat::mkdirat_exec(&cfg).unwrap();

    let mut buf = [0u8; TAG_MAX];
    let (fd, tag) = sys::shellfd::recv_fd(&receiver, &mut buf).unwrap();
    assert_eq!(tag.to_bytes(), b"dirfd");

    let st = sys::stat::fstat(&fd).unwrap();
    assert!(st.mode & 0o170000 == 0o40000, "expected directory");

    // Verify the path also exists as a directory
    let st2 = sys::stat::stat(&cpath).unwrap();
    assert_eq!(st.ino, st2.ino);
    assert_eq!(st.dev, st2.dev);

    fd.close().unwrap();
    receiver.close().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}
