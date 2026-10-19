#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec::Vec;

use builtins::error::BuiltinError;
use sys::{Origin, ShortCStr, Trace};

use crate::child::Ctx;
use crate::state::{FdVar, ShellState};
use std::format;
use std::ops::Deref;
use std::path::PathBuf;

use super::handle_readlink;
use super::parse::{Target, readlink_parse};

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

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

/// A pid + counter scratch dir (old `fdshell-readlink-<pid>.<name>` files
/// live inside it now), removed when the test's guard drops.
fn scratch() -> Scratch {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-readlink-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// A temp symlink `link` -> `target` with its own fd var `%f` (O_PATH fd on
/// the link itself). Returns the state plus the scratch dir's drop guard.
fn state_with_symlink() -> (ShellState, Scratch) {
    let dir = scratch();
    let target = dir.join("target");
    std::fs::write(&target, b"x").unwrap();
    let link = dir.join("link");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink("target-name", &link).unwrap();
    let c = std::ffi::CString::new(link.to_str().unwrap()).unwrap();
    let fd = sys::openat2::open(
        c.as_c_str(),
        sys::fcntl::O_PATH | sys::fcntl::O_NOFOLLOW | sys::fcntl::O_CLOEXEC,
    )
    .unwrap();
    let mut state = ShellState::new();
    state.fds.insert(
        c"f".into(),
        FdVar {
            fd,
            trace: Trace::boundary(Origin::Shell),
        },
    );
    (state, dir)
}

/// A temp dir holding a `g.txt` -> `target-name` symlink, registered as fd
/// var `%d`; O_PATH|O_DIRECTORY keeps the handle on the dir itself. Returns
/// the state plus the scratch dir's drop guard.
fn state_with_dir_link() -> (ShellState, Scratch) {
    let dir = scratch();
    let _ = std::fs::remove_file(dir.join("g.txt"));
    std::os::unix::fs::symlink("target-name", dir.join("g.txt")).unwrap();
    let c = std::ffi::CString::new(dir.to_str().unwrap()).unwrap();
    let fd = sys::openat2::open(
        c.as_c_str(),
        sys::fcntl::O_PATH | sys::fcntl::O_DIRECTORY | sys::fcntl::O_CLOEXEC,
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
    (state, dir)
}

/// A temp symlink whose target is 4095 bytes — the kernel's maximum —
/// registered as fd var `%f`. Returns the state plus the scratch dir's drop
/// guard.
fn state_with_long_link() -> (ShellState, Scratch) {
    let dir = scratch();
    let target = "a".repeat(4095);
    let link = dir.join("longlink");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let c = std::ffi::CString::new(link.to_str().unwrap()).unwrap();
    let fd = sys::openat2::open(
        c.as_c_str(),
        sys::fcntl::O_PATH | sys::fcntl::O_NOFOLLOW | sys::fcntl::O_CLOEXEC,
    )
    .unwrap();
    let mut state = ShellState::new();
    state.fds.insert(
        c"f".into(),
        FdVar {
            fd,
            trace: Trace::boundary(Origin::Shell),
        },
    );
    (state, dir)
}

#[test]
fn path_form_defaults() {
    with_refs(&["a.txt"], |refs, origs| {
        match readlink_parse(refs, origs).unwrap() {
            Target::Path { path, dir: None } => assert_eq!(path, c"a.txt"),
            other => panic!("unexpected {other:?}"),
        }
    });
}

#[test]
fn path_form_dir() {
    with_refs(&["a.txt", "--dir", "%d"], |refs, origs| {
        assert!(matches!(
            readlink_parse(refs, origs).unwrap(),
            Target::Path {
                dir: Some(d),
                ..
            } if d.as_bytes().unwrap() == b"d"
        ));
    });
    with_refs(&["a.txt", "--dir=%d"], |refs, origs| {
        assert!(matches!(
            readlink_parse(refs, origs).unwrap(),
            Target::Path { dir: Some(_), .. }
        ));
    });
}

#[test]
fn fd_form() {
    with_refs(&["%f"], |refs, origs| {
        match readlink_parse(refs, origs).unwrap() {
            Target::Fd { var } => assert_eq!(var.as_bytes().unwrap(), b"f"),
            other => panic!("unexpected {other:?}"),
        }
    });
}

#[test]
fn fd_form_rejects_dir() {
    with_refs(&["%f", "--dir", "%d"], |refs, origs| {
        let e = readlink_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "--dir"));
    });
}

#[test]
fn unknown_flag_rejected() {
    for args in [
        &["a.txt", "--bogus"][..],
        &["%f", "extra"][..],
        &["a.txt", "b.txt"][..],
    ] {
        with_refs(args, |refs, origs| {
            let e = readlink_parse(refs, origs).unwrap_err();
            assert!(is_invalid(&e, "flag"), "{args:?}");
        });
    }
}

#[test]
fn dir_var_must_be_prefixed() {
    for args in [&["a.txt", "--dir", "d"][..], &["a.txt", "--dir", "%"][..]] {
        with_refs(args, |refs, origs| {
            let e = readlink_parse(refs, origs).unwrap_err();
            assert!(is_invalid(&e, "dir var"), "{args:?}");
        });
    }
}

#[test]
fn duplicate_dir_rejected() {
    with_refs(&["a.txt", "--dir", "%d", "--dir", "%e"], |refs, origs| {
        let e = readlink_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "--dir"));
    });
}

#[test]
fn missing_path_errors() {
    with_refs(&[], |refs, origs| {
        let e = readlink_parse(refs, origs).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::MissingArgument("path")
        ));
    });
}

#[test]
fn empty_path_rejected() {
    with_refs(&[""], |refs, origs| {
        let e = readlink_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "path"));
    });
}

#[test]
fn help() {
    with_refs(&["--help"], |refs, origs| {
        let e = readlink_parse(refs, origs).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Help));
    });
}

#[test]
fn handler_fd_form_reads_the_link() {
    let (state, _scratch) = state_with_symlink();
    with_refs(&["%f"], |refs, origs| {
        assert_eq!(
            handle_readlink(&Ctx::new(c"readlink".into(), refs, origs, &state)).unwrap(),
            0
        );
    });
}

#[test]
fn handler_non_link_fd_is_syscall_error() {
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
        let e = handle_readlink(&Ctx::new(c"readlink".into(), refs, origs, &state)).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Syscall));
    });
}

#[test]
fn handler_unset_fd_var() {
    let state = ShellState::new();
    with_refs(&["%missing"], |refs, origs| {
        let e = handle_readlink(&Ctx::new(c"readlink".into(), refs, origs, &state)).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::FdVarNotFound));
    });
}

#[test]
fn handler_unset_dir_var() {
    let state = ShellState::new();
    with_refs(&["a.txt", "--dir", "%missing"], |refs, origs| {
        let e = handle_readlink(&Ctx::new(c"readlink".into(), refs, origs, &state)).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::FdVarNotFound));
    });
}

#[test]
fn handler_dir_var_resolves_against_fd() {
    let (state, _scratch) = state_with_dir_link();
    with_refs(&["g.txt", "--dir", "%d"], |refs, origs| {
        assert_eq!(
            handle_readlink(&Ctx::new(c"readlink".into(), refs, origs, &state)).unwrap(),
            0
        );
    });
}

#[test]
fn handler_long_target_fits_the_buffer() {
    let (state, _scratch) = state_with_long_link();
    with_refs(&["%f"], |refs, origs| {
        assert_eq!(
            handle_readlink(&Ctx::new(c"readlink".into(), refs, origs, &state)).unwrap(),
            0
        );
    });
}

#[test]
fn handler_missing_path_is_syscall_error() {
    let state = ShellState::new();
    with_refs(&["no-such-file-xyz"], |refs, origs| {
        let e = handle_readlink(&Ctx::new(c"readlink".into(), refs, origs, &state)).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Syscall));
    });
}
