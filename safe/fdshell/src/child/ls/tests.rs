#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec::Vec;

use builtins::error::BuiltinError;
use sys::{Origin, ShortCStr, Trace};

use crate::state::{FdVar, ShellState};
use std::format;

use super::handle_ls;
use super::parse::{Target, ls_parse};

fn with_refs<R, F>(args: &[&str], f: F) -> R
where
    F: FnOnce(&[&core::ffi::CStr], &[ShortCStr]) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let origs: Vec<ShortCStr> = args
        .iter()
        .map(|a| ShortCStr::from_vec(a.as_bytes().to_vec()).unwrap())
        .collect();
    f(&refs, &origs)
}

fn is_invalid(e: &error_stack::Report<BuiltinError>, what: &'static str) -> bool {
    matches!(e.current_context(), BuiltinError::InvalidArgument(s) if *s == what)
}

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("fdshell-ls-{}", name))
}

/// A temp directory registered as fd var `%d` (opened read-only, O_DIRECTORY).
fn state_with_dir() -> ShellState {
    let dir = tmp("dir");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("entry"), b"x").unwrap();
    let c = std::ffi::CString::new(dir.to_str().unwrap()).unwrap();
    let fd = sys::openat2::open(
        c.as_c_str(),
        sys::fcntl::O_RDONLY | sys::fcntl::O_DIRECTORY | sys::fcntl::O_CLOEXEC,
    )
    .unwrap();
    let mut state = ShellState::new();
    state.fds.insert(
        c"d".into(),
        FdVar {
            fd,
            trace: Trace::boundary(Origin::Shell),
        },
    );
    state
}

#[test]
fn path_form_defaults() {
    with_refs(&["sub"], |refs, origs| {
        match ls_parse(refs, origs).unwrap() {
            Target::Path { path, dir: None } => assert_eq!(path.to_bytes(), b"sub"),
            other => panic!("unexpected {other:?}"),
        }
    });
}

#[test]
fn path_form_dir() {
    with_refs(&["sub", "--dir", "%d"], |refs, origs| {
        assert!(matches!(
            ls_parse(refs, origs).unwrap(),
            Target::Path { dir: Some(d), .. } if d.as_bytes().unwrap() == b"d"
        ));
    });
    with_refs(&["sub", "--dir=%d"], |refs, origs| {
        assert!(matches!(
            ls_parse(refs, origs).unwrap(),
            Target::Path { dir: Some(_), .. }
        ));
    });
}

#[test]
fn fd_form() {
    with_refs(&["%d"], |refs, origs| {
        match ls_parse(refs, origs).unwrap() {
            Target::Fd { var } => assert_eq!(var.as_bytes().unwrap(), b"d"),
            other => panic!("unexpected {other:?}"),
        }
    });
}

#[test]
fn fd_form_rejects_dir() {
    with_refs(&["%d", "--dir", "%d"], |refs, origs| {
        let e = ls_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "--dir"));
    });
}

#[test]
fn unknown_flag_rejected() {
    for args in [
        &["sub", "--bogus"][..],
        &["%d", "extra"][..],
        &["sub", "other"][..],
    ] {
        with_refs(args, |refs, origs| {
            let e = ls_parse(refs, origs).unwrap_err();
            assert!(is_invalid(&e, "flag"), "{args:?}");
        });
    }
}

#[test]
fn dir_var_must_be_prefixed() {
    for args in [&["sub", "--dir", "d"][..], &["sub", "--dir", "%"][..]] {
        with_refs(args, |refs, origs| {
            let e = ls_parse(refs, origs).unwrap_err();
            assert!(is_invalid(&e, "dir var"), "{args:?}");
        });
    }
}

#[test]
fn duplicate_dir_rejected() {
    with_refs(&["sub", "--dir", "%d", "--dir", "%e"], |refs, origs| {
        let e = ls_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "--dir"));
    });
}

#[test]
fn missing_path_errors() {
    with_refs(&[], |refs, origs| {
        let e = ls_parse(refs, origs).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::MissingArgument("path")
        ));
    });
}

#[test]
fn empty_path_rejected() {
    with_refs(&[""], |refs, origs| {
        let e = ls_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "path"));
    });
}

#[test]
fn help() {
    with_refs(&["--help"], |refs, origs| {
        let e = ls_parse(refs, origs).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Help));
    });
}

#[test]
fn handler_fd_form_lists_the_dir() {
    let state = state_with_dir();
    with_refs(&["%d"], |refs, origs| {
        assert_eq!(handle_ls(c"ls".into(), refs, origs, &state).unwrap(), 0);
    });
}

#[test]
fn handler_path_form_lists_the_dir() {
    let dir = tmp("pathform");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    with_refs(&[dir.to_str().unwrap()], |refs, origs| {
        assert_eq!(
            handle_ls(c"ls".into(), refs, origs, &ShellState::new()).unwrap(),
            0
        );
    });
}

#[test]
fn handler_unset_fd_var() {
    with_refs(&["%missing"], |refs, origs| {
        let e = handle_ls(c"ls".into(), refs, origs, &ShellState::new()).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::FdVarNotFound));
    });
}

#[test]
fn handler_non_directory_is_syscall_error() {
    let mut state = ShellState::new();
    let fd = sys::memfd::memfd_create().unwrap();
    state.fds.insert(
        c"f".into(),
        FdVar {
            fd,
            trace: Trace::boundary(Origin::Shell),
        },
    );
    with_refs(&["%f"], |refs, origs| {
        let e = handle_ls(c"ls".into(), refs, origs, &state).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Syscall));
    });
}

#[test]
fn handler_missing_path_is_syscall_error() {
    with_refs(&["no-such-dir-xyz"], |refs, origs| {
        let e = handle_ls(c"ls".into(), refs, origs, &ShellState::new()).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Syscall));
    });
}
