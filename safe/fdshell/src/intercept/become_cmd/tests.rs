#![allow(clippy::unwrap_used)]
use super::*;
use crate::redirect::RedirectDef;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. The fixed name
/// `fdshell_exec_redirect_test` had no cleanup at all: concurrent runs shared
/// it and pid recycling inherited the old file (LESSONS, task #255; sweep
/// task #342). The file lives inside this dir now. Removal is `.ok()`-tolerant:
/// teardown never panics, and it runs on a failing test's unwind, which is
/// exactly the test that used to leak.
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

/// A pid + counter scratch dir `fdshell-become_cmd-<tag>-<pid>-<counter>`,
/// removed when the test's guard drops.
fn scratch(tag: &str) -> Scratch {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fdshell-become_cmd-{tag}-{}-{}",
        std::process::id(),
        c
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

fn make_exec_cmdline(redirects: Vec<RedirectDef>) -> CommandLine {
    CommandLine {
        prefix: crate::parse::BuiltinPrefix::None,
        command: c"exec".into(),
        command_mask: vec![],
        command_span: vec![],
        command_quoted: false,
        env_assigns: vec![],
        args: vec![],
        args_mask: vec![],
        args_span: vec![],
        args_quoted: vec![],
        captures: vec![],
        redirects,
        pidvar: None,
        bg_force: false,
    }
}

#[test]
fn apply_redirects_dup2s_var_fd_onto_target() {
    let dir = scratch("redirect");
    let path = dir.join("f");
    let cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let fd = sys::openat2::open(
        cstr.as_c_str(),
        sys::fcntl::O_RDWR + sys::fcntl::O_CREAT + sys::fcntl::O_TRUNC,
    )
    .unwrap();
    let cell = ForkCell::new({
        let mut state = ShellState::new();
        state.fds.insert(
            c"x".into(),
            crate::state::FdVar {
                fd,
                trace: sys::Trace::boundary(sys::Origin::Shell),
            },
        );
        state
    });
    let cmdline = make_exec_cmdline(vec![RedirectDef::var(99, c"x")]);
    apply_redirects(&cmdline, &cell).unwrap();
    // fd 99 must now point at the var's open file.
    let link = std::fs::read_link("/proc/self/fd/99").unwrap();
    assert_eq!(link, path);
}

#[test]
fn apply_redirects_unknown_var_is_error() {
    let cell = ForkCell::new(ShellState::new());
    let cmdline = make_exec_cmdline(vec![RedirectDef::var(99, c"nope")]);
    let report = apply_redirects(&cmdline, &cell).unwrap_err();
    assert!(matches!(report.current_context(), CmdError::Redirect));
}

#[test]
fn apply_redirects_missing_path_is_error() {
    let cell = ForkCell::new(ShellState::new());
    let cmdline = make_exec_cmdline(vec![RedirectDef::write_path(99, c"/nonexistent-dir-xyz/f")]);
    let report = apply_redirects(&cmdline, &cell).unwrap_err();
    assert!(matches!(report.current_context(), CmdError::Redirect));
}
