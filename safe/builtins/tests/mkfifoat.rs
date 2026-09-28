#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;
use std::sync::atomic::AtomicU64;
use sys::shellfd::TAG_MAX;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], expected: BuiltinError) {
    with_args(args, |a| {
        match builtins::mkfifoat::parse::mkfifoat_parse(a) {
            Err(e) => {
                let ctx = e.current_context();
                match (ctx, expected) {
                    (BuiltinError::Help, BuiltinError::Help) => {}
                    (BuiltinError::InvalidArgument(_), BuiltinError::InvalidArgument(_)) => {}
                    _ => panic!("unexpected error: {ctx}"),
                }
            }
            _ => panic!("expected Err"),
        }
    });
}

fn assert_invalid_arg(args: &[&str]) {
    assert_err(args, BuiltinError::InvalidArgument("x"));
}

fn assert_ok<F: FnOnce(&builtins::mkfifoat::parse::MkfifoatConfig)>(args: &[&str], f: F) {
    with_args(args, |a| {
        match builtins::mkfifoat::parse::mkfifoat_parse(a) {
            Ok(cfg) => f(&cfg),
            Err(e) => panic!("expected Ok, got Err({e})"),
        }
    });
}

#[test]
fn basic() {
    assert_ok(&["--mode", "600", "pipe"], |cfg| {
        assert!(cfg.dirfd.is_none());
        assert_eq!(cfg.mode, 0o600);
        assert_eq!(cfg.path.to_bytes(), b"pipe");
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
fn short_flag_no_path() {
    assert_invalid_arg(&["-x"])
}

#[test]
fn missing_path() {
    assert_invalid_arg(&["--mode", "600"])
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
fn mode_octal() {
    assert_ok(&["--mode", "600", "x"], |cfg| {
        assert_eq!(cfg.mode, 0o600);
    });
}

#[test]
fn mode_hex() {
    assert_ok(&["--mode", "0x180", "x"], |cfg| {
        assert_eq!(cfg.mode, 0o600);
    });
}

#[test]
fn mode_octal_prefix() {
    assert_ok(&["--mode", "0o600", "x"], |cfg| {
        assert_eq!(cfg.mode, 0o600);
    });
}

#[test]
fn mode_overflow() {
    assert_invalid_arg(&["--mode", "0x100000000", "x"])
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
fn resolve_no_magiclinks() {
    assert_ok(&["--resolve", "RESOLVE_NO_MAGICLINKS", "x"], |cfg| {
        assert_eq!(cfg.resolve, 2);
    });
}

#[test]
fn resolve_no_xdev() {
    assert_ok(&["--resolve", "RESOLVE_NO_XDEV", "x"], |cfg| {
        assert_eq!(cfg.resolve, 4);
    });
}

#[test]
fn resolve_cached() {
    assert_ok(&["--resolve", "RESOLVE_CACHED", "x"], |cfg| {
        assert_eq!(cfg.resolve, 32);
    });
}

#[test]
fn resolve_hex() {
    assert_ok(&["--resolve", "0xff", "x"], |cfg| {
        assert_eq!(cfg.resolve, 255);
    });
}

#[test]
fn eq_syntax() {
    assert_ok(&["--mode=600", "x"], |cfg| {
        assert_eq!(cfg.mode, 0o600);
    });
}

/// A scratch dir with pid + atomic counter: tests in this binary share the
/// process and run on parallel threads (LESSONS: pid-only paths collide).
fn test_dir() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("fdshell-mkfifoat-{}-{}", std::process::id(), c))
}

/// Runs `mkfifoat_exec` on `args` and returns the received handle, asserting
/// the capture tag.
fn run_exec(args: &[&CStr], expected_tag: &[u8]) -> sys::LocalFd {
    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    drop(shell_a);

    let cfg = builtins::mkfifoat::parse::mkfifoat_parse(args).unwrap();
    builtins::mkfifoat::mkfifoat_exec(&cfg, &shell_sock).unwrap();

    let mut buf = [0u8; TAG_MAX];
    let (fd, tag) = receiver
        .recv_fd(&mut buf, sys::Pid::from_raw(std::process::id() as i32))
        .unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), expected_tag);
    drop(receiver);
    fd
}

/// `mkfifoat` without `--dirfd` resolves against the CWD (the `AtFd::cwd()`
/// arm) and sends a handle tagged `fifo` that is the created FIFO.
#[test]
fn mkfifoat_exec_sends_fifo_handle() {
    let dir = test_dir();
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("pipe");
    let cpath = CString::new(path.to_str().unwrap()).unwrap();

    let mode_arg = CString::from(c"--mode");
    let mode_val = CString::from(c"600");
    let args = [mode_arg.as_c_str(), mode_val.as_c_str(), cpath.as_c_str()];
    let fd = run_exec(&args, b"fifo");

    let st = fd.fstat().unwrap();
    assert_eq!(
        st.mode & sys::stat::S_IFMT,
        sys::stat::S_IFIFO,
        "expected fifo"
    );

    // The handle must point at the same object as the path on disk.
    let st2 = sys::stat::stat(&cpath).unwrap();
    assert_eq!(st.ino, st2.ino);
    assert_eq!(st.dev, st2.dev);

    drop(fd);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// With `--dirfd`, the FIFO is created inside the given directory (the
/// `ImportedFd::at` arm).
#[test]
fn mkfifoat_exec_with_dirfd() {
    let dir = test_dir();
    std::fs::create_dir(&dir).unwrap();
    let cdir = CString::new(dir.to_str().unwrap()).unwrap();
    // O_DIRECTORY (O_RDONLY is 0) so the handle is a directory fd.
    let dir_local = sys::openat2::open(&cdir, sys::fcntl::O_DIRECTORY).unwrap();
    // `--dirfd` takes a borrowed (CLOEXEC-clear) fd number.
    let dir_fd = dir_local.export().unwrap();

    let dirfd_arg = CString::from(c"--dirfd");
    let dirfd_val = CString::new(dir_fd.as_raw().to_string()).unwrap();
    let mode_arg = CString::from(c"--mode");
    let mode_val = CString::from(c"600");
    let name = CString::from(c"pipe");
    let args = [
        dirfd_arg.as_c_str(),
        dirfd_val.as_c_str(),
        mode_arg.as_c_str(),
        mode_val.as_c_str(),
        name.as_c_str(),
    ];
    let fd = run_exec(&args, b"fifo");

    let st = fd.fstat().unwrap();
    assert_eq!(
        st.mode & sys::stat::S_IFMT,
        sys::stat::S_IFIFO,
        "expected fifo"
    );

    let cpath = CString::new(dir.join("pipe").to_str().unwrap()).unwrap();
    let st2 = sys::stat::stat(&cpath).unwrap();
    assert_eq!(st.ino, st2.ino);
    assert_eq!(st.dev, st2.dev);

    drop(fd);
    // `dir_fd` is a non-owning export (like the mkdirat test's); its dup is
    // closed when the test process exits. Only the owning `dir_local` drops.
    drop(dir_local);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Special bits (setuid/setgid/sticky) are stripped: only `mode & 0o777` is
/// passed to `mkfifoat`. 0o600 keeps its value under any umask, so the
/// equality holds in any environment.
#[test]
fn mkfifoat_exec_masks_special_bits() {
    let dir = test_dir();
    std::fs::create_dir(&dir).unwrap();
    let cpath = CString::new(dir.join("pipe").to_str().unwrap()).unwrap();

    let mode_arg = CString::from(c"--mode");
    let mode_val = CString::from(c"0o1600");
    let args = [mode_arg.as_c_str(), mode_val.as_c_str(), cpath.as_c_str()];
    let fd = run_exec(&args, b"fifo");

    let st = fd.fstat().unwrap();
    assert_eq!(st.mode & 0o7777, 0o600, "special bits must be stripped");

    drop(fd);
    std::fs::remove_dir_all(&dir).unwrap();
}
