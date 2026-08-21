#![allow(clippy::unwrap_used)]
use super::*;
use crate::redirect::RedirectDef;
use alloc::vec;
use alloc::vec::Vec;

fn make_exec_cmdline(redirects: Vec<RedirectDef>) -> CommandLine {
    CommandLine {
        builtin: false,
        command: c"exec".into(),
        args: vec![],
        args_fq: vec![],
        captures: vec![],
        redirects,
        pidvar: None,
        bg_force: false,
    }
}

#[test]
fn apply_redirects_dup2s_var_fd_onto_target() {
    let path = std::env::temp_dir().join("fdshell_exec_redirect_test");
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
