#![allow(clippy::unwrap_used)]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::AtomicU64;
use sys::fcntl::{O_DIRECTORY, O_PATH};
use sys::siginfo::WaitStatus;

const EXEC_OK: &str = env!("CARGO_BIN_EXE_builtin_exec_ok");

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_parse_err(args: &[&str]) {
    with_args(args, |a| match builtins::execat::parse::execat_parse(a) {
        Err(e) => {
            let ctx = e.current_context();
            match ctx {
                BuiltinError::Help | BuiltinError::InvalidArgument(_) => {}
                other => panic!("unexpected error: {other}"),
            }
        }
        _ => panic!("expected Err"),
    });
}

fn assert_parse_ok<F: FnOnce(&builtins::execat::parse::ExecAtConfig)>(args: &[&str], f: F) {
    with_args(args, |a| match builtins::execat::parse::execat_parse(a) {
        Ok(cfg) => f(&cfg),
        Err(e) => panic!("expected Ok, got Err({e})"),
    });
}

#[test]
fn empty_args() {
    assert_parse_err(&[]);
}

#[test]
fn help_long() {
    assert_parse_err(&["--help"]);
}

#[test]
fn missing_percent() {
    assert_parse_err(&["CWD", "bin"]);
}

#[test]
fn empty_var_name() {
    assert_parse_err(&["%", "bin"]);
}

#[test]
fn missing_pathname() {
    assert_parse_err(&["%CWD"]);
}

#[test]
fn empty_pathname() {
    assert_parse_err(&["%CWD", ""]);
}

#[test]
fn var_pathname_argv() {
    assert_parse_ok(&["%CWD", "bin/x", "a", "b"], |cfg| {
        assert_eq!(cfg.var.to_bytes(), b"%CWD");
        assert_eq!(cfg.pathname.to_bytes(), b"bin/x");
    });
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn test_dir() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("fdshell-execat-test-{}-{}", std::process::id(), c))
}

fn open_test_dir(dir: &std::path::Path) -> sys::LocalFd {
    let cs = CString::new(dir.to_str().unwrap()).unwrap();
    sys::openat2::open(&cs, O_PATH + O_DIRECTORY).unwrap()
}

fn exec_child(f: impl FnOnce()) {
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            f();
            sys::exit(1);
        }
        Some(pidfd) => {
            let status = sys::wait_pidfd::wait_pidfd(&pidfd).unwrap();
            match status {
                WaitStatus::Exited(42) => {}
                other => panic!("unexpected status {}", other.exit_code()),
            }
        }
    }
}

#[test]
fn execat_exec_ok() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("mybin");
    std::fs::copy(EXEC_OK, &target).unwrap();
    {
        let mut perms = std::fs::metadata(&target).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&target, perms).unwrap();
    }
    let dirfd = open_test_dir(&dir);
    exec_child(|| {
        match builtins::execat::execat_exec(dirfd.at(), c"mybin", &[c"builtin_exec_ok"], &[]) {
            Ok(()) => {}
            Err(_) => sys::exit(1),
        }
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn execat_exec_missing_fails() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let dirfd = open_test_dir(&dir);
    exec_child(|| {
        let r = builtins::execat::execat_exec(dirfd.at(), c"nope-xxxxxxxx", &[c"x"], &[]);
        if r.is_ok() {
            sys::exit(1);
        } else {
            sys::exit(42);
        }
    });
    let _ = std::fs::remove_dir_all(&dir);
}
