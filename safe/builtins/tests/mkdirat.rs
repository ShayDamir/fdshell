#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use sys::shellfd::TAG_MAX;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). Removal is `.ok()`-tolerant: teardown never panics, and it runs
/// on a failing test's unwind, which is exactly the test that used to leak.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A pid + counter scratch dir named after `tag` (the old
/// `fdshell-test-mkdirat-<tag>-…` prefixes), removed when the test's guard
/// drops.
fn scratch(tag: &str) -> Scratch {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fdshell-test-mkdirat-{tag}-{}-{}",
        std::process::id(),
        c
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], expected: BuiltinError) {
    with_args(args, |a| {
        match builtins::mkdirat::parse::mkdirat_parse(a, false) {
            Err(e) => {
                let ctx = e.current_context();
                match (ctx, expected) {
                    (BuiltinError::Help, BuiltinError::Help) => {}
                    (BuiltinError::InvalidArgument(_), BuiltinError::InvalidArgument(_)) => {}
                    (BuiltinError::ModeReopenDenied, BuiltinError::ModeReopenDenied) => {}
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

fn assert_ok<F: FnOnce(&builtins::mkdirat::parse::MkdiratConfig)>(args: &[&str], f: F) {
    with_args(args, |a| {
        match builtins::mkdirat::parse::mkdirat_parse(a, false) {
            Ok(cfg) => f(&cfg),
            Err(e) => panic!("expected Ok, got Err({e})"),
        }
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
fn short_flag_no_path() {
    assert_invalid_arg(&["-x"])
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
fn mode_overflow() {
    assert_invalid_arg(&["--mode", "0x100000000", "x"])
}

/// A mode-less invocation gets `mkdir(1)`'s default, so the builtin's own
/// reopen of the new directory succeeds (the reported mode-0 trap).
#[test]
fn mode_default_is_the_dir_mode() {
    assert_ok(&["newdir"], |cfg| {
        assert_eq!(cfg.mode, builtins::mode::DIR_MODE);
    });
}

#[test]
fn mode_boundary_owner_read_execute_is_accepted() {
    assert_ok(&["--mode", "500", "x"], |cfg| {
        assert_eq!(cfg.mode, 0o500);
    });
    assert_ok(&["--mode", "700", "x"], |cfg| {
        assert_eq!(cfg.mode, 0o700);
    });
}

/// The unit-test cache holds the umask at 0, so these are pure bit checks:
/// a mode without the owner `x` (or `r`) bit is refused before `mkdirat` runs.
#[test]
fn mode_without_owner_read_execute_is_denied() {
    assert_err(&["--mode", "400", "x"], BuiltinError::ModeReopenDenied);
    assert_err(&["--mode", "100", "x"], BuiltinError::ModeReopenDenied);
    assert_err(&["--mode", "077", "x"], BuiltinError::ModeReopenDenied);
}

#[test]
fn explicit_mode_zero_is_denied() {
    assert_err(&["--mode", "0", "x"], BuiltinError::ModeReopenDenied);
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
    assert_ok(&["--mode=755", "x"], |cfg| {
        assert_eq!(cfg.mode, 0o755);
    });
}

#[test]
fn test_mkdirat_exec() {
    let dir = scratch("exec");
    let subdir_path = dir.join("subdir");

    let cpath = CString::new(subdir_path.to_str().unwrap()).unwrap();

    // Create a socket to use as the shell fd
    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    drop(shell_a);

    let mode_arg = CString::from(c"--mode");
    let mode_val = CString::from(c"755");
    let args = [mode_arg.as_c_str(), mode_val.as_c_str(), cpath.as_c_str()];
    let cfg = builtins::mkdirat::parse::mkdirat_parse(&args, false).unwrap();
    builtins::mkdirat::mkdirat_exec(&cfg, &shell_sock).unwrap();

    let mut buf = [0u8; TAG_MAX];
    let (fd, tag) = receiver
        .recv_fd(&mut buf, sys::Pid::from_raw(std::process::id() as i32))
        .unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), b"dirfd");

    let st = fd.fstat().unwrap();
    assert!(
        st.mode & sys::stat::S_IFMT == sys::stat::S_IFDIR,
        "expected directory"
    );

    // Verify the path also exists as a directory
    let st2 = sys::stat::stat(&cpath).unwrap();
    assert_eq!(st.ino, st2.ino);
    assert_eq!(st.dev, st2.dev);

    drop(fd);
    drop(receiver);
}

/// Regression for the mode-0 trap: `mkdirat` with no `--mode` must create the
/// directory *and* hand back the `dirfd` handle (it used to fail `EACCES` on
/// its own reopen and leave the directory on disk with no fd).
#[test]
fn test_mkdirat_exec_without_mode() {
    let dir = scratch("modeless");
    let cpath = CString::new(dir.join("subdir").to_str().unwrap()).unwrap();

    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    drop(shell_a);

    let cfg = builtins::mkdirat::parse::mkdirat_parse(&[cpath.as_c_str()], false).unwrap();
    assert_eq!(cfg.mode, builtins::mode::DIR_MODE);
    builtins::mkdirat::mkdirat_exec(&cfg, &shell_sock).unwrap();

    let mut buf = [0u8; TAG_MAX];
    let (fd, tag) = receiver
        .recv_fd(&mut buf, sys::Pid::from_raw(std::process::id() as i32))
        .unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), b"dirfd");

    let st = fd.fstat().unwrap();
    assert!(
        st.mode & sys::stat::S_IFMT == sys::stat::S_IFDIR,
        "expected directory"
    );
    // The default carries the owner `x` bit under any umask (0777 & ~umask).
    assert_eq!(st.mode & 0o100, 0o100, "owner must keep the x bit");

    drop(fd);
    drop(receiver);
}

#[test]
fn test_mkdirat_exec_masks_special_bits() {
    let dir = scratch("mask");
    let subdir_path = dir.join("subdir");

    let cpath = CString::new(subdir_path.to_str().unwrap()).unwrap();

    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    drop(shell_a);

    let mode_arg = CString::from(c"--mode");
    let mode_val = CString::from(c"0o1755");
    let args = [mode_arg.as_c_str(), mode_val.as_c_str(), cpath.as_c_str()];
    let cfg = builtins::mkdirat::parse::mkdirat_parse(&args, false).unwrap();
    builtins::mkdirat::mkdirat_exec(&cfg, &shell_sock).unwrap();

    let mut buf = [0u8; TAG_MAX];
    let (fd, _tag) = receiver
        .recv_fd(&mut buf, sys::Pid::from_raw(std::process::id() as i32))
        .unwrap();
    fd.verify().unwrap();

    let st = fd.fstat().unwrap();
    assert_eq!(st.mode & 0o7777, 0o755, "special bits must be stripped");

    drop(fd);
    drop(receiver);
}

// --- strict (capability) mode ------------------------------------------------

fn with_dirfd<R, F: FnOnce(&str) -> R>(f: F) -> R {
    let (rd, _wr) = sys::pipe::pipe2(0).unwrap();
    rd.verify().unwrap();
    let s = rd.export().unwrap().as_raw().to_string();
    f(&s)
}

/// Parses `args` in strict mode and hands the error context to `f`; panics if
/// the parse succeeds.
fn strict_parse(args: &[&str], f: impl FnOnce(&BuiltinError)) {
    with_args(args, |a| {
        match builtins::mkdirat::parse::mkdirat_parse(a, true) {
            Err(e) => f(e.current_context()),
            _ => panic!("expected Err"),
        }
    });
}

#[test]
fn strict_bans_missing_dirfd() {
    strict_parse(&["newdir"], |e| {
        assert!(matches!(e, BuiltinError::StrictRequiresDirfd));
    });
}

#[test]
fn strict_bans_absolute_path() {
    with_dirfd(|d| {
        strict_parse(&["--dirfd", d, "/abs"], |e| {
            assert!(matches!(e, BuiltinError::StrictAbsolutePath));
        });
    });
}

#[test]
fn strict_allows_explicit_dirfd_relative() {
    with_dirfd(|d| {
        with_args(&["--dirfd", d, "newdir"], |a| {
            let cfg = builtins::mkdirat::parse::mkdirat_parse(a, true).unwrap();
            assert!(cfg.dirfd.is_some());
        });
    });
}

#[test]
fn non_strict_allows_missing_dirfd_absolute() {
    with_args(&["/abs"], |a| {
        let cfg = builtins::mkdirat::parse::mkdirat_parse(a, false).unwrap();
        assert!(cfg.dirfd.is_none());
    });
}
