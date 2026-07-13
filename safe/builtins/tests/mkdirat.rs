#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;
use sys::shellfd::TAG_MAX;

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], expected: BuiltinError) {
    with_args(args, |a| match builtins::mkdirat::parse::mkdirat_parse(a) {
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
    assert_invalid_arg(&["--bad", "x"])
}

#[test]
fn missing_path() {
    assert_invalid_arg(&["--mode", "755"])
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
    assert_invalid_arg(&["--mode"])
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
    let dupfd = rd.export().unwrap();
    dupfd.verify().unwrap();
    let s = format!("{}", dupfd.as_raw());
    assert_ok(&["--dirfd", &s, "x"], |cfg| {
        assert_eq!(cfg.dirfd.as_ref().map(|d| d.as_raw()), Some(dupfd.as_raw()));
    });
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

    // Create a socket to use as the shell fd (fd 3)
    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export_to(3).unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    shell_a.try_close().unwrap();

    let mode_arg = CString::from(c"--mode");
    let mode_val = CString::from(c"755");
    let args = [mode_arg.as_c_str(), mode_val.as_c_str(), cpath.as_c_str()];
    let cfg = builtins::mkdirat::parse::mkdirat_parse(&args).unwrap();
    builtins::mkdirat::mkdirat_exec(&cfg, &shell_sock).unwrap();

    let mut buf = [0u8; TAG_MAX];
    let (fd, tag) = sys::shellfd::recv_fd(&receiver, &mut buf, std::process::id() as i32).unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), b"dirfd");

    let st = sys::stat::fstat(&fd).unwrap();
    assert!(
        st.mode & sys::stat::S_IFMT == sys::stat::S_IFDIR,
        "expected directory"
    );

    // Verify the path also exists as a directory
    let st2 = sys::stat::stat(&cpath).unwrap();
    assert_eq!(st.ino, st2.ino);
    assert_eq!(st.dev, st2.dev);

    fd.try_close().unwrap();
    receiver.try_close().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}
