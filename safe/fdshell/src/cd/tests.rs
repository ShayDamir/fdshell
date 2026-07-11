#![cfg_attr(test, allow(clippy::unwrap_used))]
use super::*;
use crate::state::ShellState;
use sys::siginfo::WaitStatus;

fn child_test(f: impl FnOnce()) {
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            f();
            std::process::exit(42);
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
fn cd_to_tmp() {
    child_test(|| {
        let mut state = ShellState::new();
        let tmp = c"/tmp".into();
        cd(&[tmp], &mut state).unwrap();
        let cwd = state.fds.get(&c"CWD".into()).unwrap();
        cwd.verify().unwrap();
    });
}

#[test]
fn cd_to_home() {
    let home = std::env::var_os("HOME");
    let Some(home_path) = home else {
        child_test(|| {
            let mut state = ShellState::new();
            let e = cd(&[], &mut state).unwrap_err();
            assert!(matches!(e.current_context(), CdError::HomeNotSet));
        });
        return;
    };
    if std::path::Path::new(&home_path).exists() {
        child_test(|| {
            let mut state = ShellState::new();
            cd(&[], &mut state).unwrap();
            let cwd = state.fds.get(&c"CWD".into()).unwrap();
            cwd.verify().unwrap();
        });
    } else {
        child_test(|| {
            let mut state = ShellState::new();
            let e = cd(&[], &mut state).unwrap_err();
            assert!(matches!(e.current_context(), CdError::CdPathOpen));
        });
    }
}

#[test]
fn cd_to_self() {
    child_test(|| {
        let mut state = ShellState::new();
        let tmp = c"/tmp".into();
        cd(&[tmp], &mut state).unwrap();
        let cwd_fd = state.fds.get(&c"CWD".into()).unwrap().try_clone().unwrap();
        state.fds.insert(c"CWD".into(), cwd_fd);
        cd(&[c"%CWD".into()], &mut state).unwrap();
        let cwd = state.fds.get(&c"CWD".into()).unwrap();
        cwd.verify().unwrap();
    });
}

#[test]
fn cd_missing_path() {
    child_test(|| {
        let mut state = ShellState::new();
        let bad = c"/nonexistent-cd-test-xxxxxxxx".into();
        let e = cd(&[bad], &mut state).unwrap_err();
        assert!(matches!(e.current_context(), CdError::CdPathOpen));
    });
}

#[test]
fn cd_missing_var() {
    child_test(|| {
        let mut state = ShellState::new();
        let bad = c"%NONEXISTENT".into();
        let e = cd(&[bad], &mut state).unwrap_err();
        assert!(matches!(e.current_context(), CdError::FdNotSet));
    });
}

#[test]
fn cd_dash_switches_to_oldpwd() {
    child_test(|| {
        let mut state = ShellState::new();
        let tmp = c"/tmp".into();
        cd(&[tmp], &mut state).unwrap();
        let root = c"/".into();
        cd(&[root], &mut state).unwrap();
        let dash = c"-".into();
        cd(&[dash], &mut state).unwrap();
        let cwd = state.fds.get(&c"CWD".into()).unwrap();
        cwd.verify().unwrap();
        let old = state.fds.get(&c"OLDCWD".into()).unwrap();
        old.verify().unwrap();
    });
}

#[test]
fn cd_move_cwd_to_oldcwd() {
    child_test(|| {
        let mut state = ShellState::new();
        let tmp = c"/tmp".into();
        cd(&[tmp], &mut state).unwrap();
        assert!(state.fds.contains_key(&c"CWD".into()));
        assert!(!state.fds.contains_key(&c"OLDCWD".into()));
        let root = c"/".into();
        cd(&[root], &mut state).unwrap();
        assert!(state.fds.contains_key(&c"CWD".into()));
        assert!(state.fds.contains_key(&c"OLDCWD".into()));
    });
}
