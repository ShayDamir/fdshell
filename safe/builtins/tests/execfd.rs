#![allow(clippy::unwrap_used)]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;
use std::sync::atomic::AtomicU64;
use sys::fcntl::O_PATH;
use sys::siginfo::WaitStatus;

const EXEC_OK: &str = env!("CARGO_BIN_EXE_builtin_exec_ok");

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_parse_err(args: &[&str]) {
    with_args(args, |a| match builtins::execfd::parse::execfd_parse(a) {
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

fn assert_parse_ok<F: FnOnce(&builtins::execfd::parse::ExecFdConfig)>(args: &[&str], f: F) {
    with_args(args, |a| match builtins::execfd::parse::execfd_parse(a) {
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
fn help_short() {
    assert_parse_err(&["-h"]);
}

#[test]
fn missing_percent() {
    assert_parse_err(&["CWD"]);
}

#[test]
fn empty_var_name() {
    assert_parse_err(&["%"]);
}

#[test]
fn var_only() {
    assert_parse_ok(&["%CWD"], |cfg| {
        assert_eq!(cfg.var.to_bytes(), b"%CWD");
    });
}

#[test]
fn var_with_program_args() {
    assert_parse_ok(&["%fd", "echo", "hello"], |cfg| {
        assert_eq!(cfg.var.to_bytes(), b"%fd");
    });
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn test_dir() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("fdshell-execfd-test-{}-{}", std::process::id(), c))
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
fn execfd_exec_ok() {
    let path = CString::new(EXEC_OK).unwrap();
    let fd = sys::openat2::open(&path, O_PATH).unwrap();
    exec_child(
        || match builtins::execfd::execfd_exec(&fd, &[c"builtin_exec_ok"], &[]) {
            Ok(()) => {}
            Err(_) => sys::exit(1),
        },
    );
}

#[test]
fn execfd_exec_nonexecutable_fails() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let script_path = dir.join("noscript.sh");
    std::fs::write(&script_path, b"#!/bin/sh\nexit 7\n").unwrap();
    let cs = CString::new(script_path.to_str().unwrap()).unwrap();
    let fd = sys::openat2::open(&cs, O_PATH).unwrap();
    exec_child(
        || match builtins::execfd::execfd_exec(&fd, &[c"noscript.sh"], &[]) {
            Ok(()) => sys::exit(1),
            Err(_) => sys::exit(42),
        },
    );
    let _ = std::fs::remove_dir_all(&dir);
}
