#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::state::ShellState;
use alloc::format;
use alloc::vec::Vec;
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

/// Fork a child (exits 0) and register it in `state.tasks` under `name`.
fn spawn_task(state: &mut ShellState, name: &[u8]) -> sys::Pid {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => sys::exit(0),
        Some(pidfd) => {
            state.tasks.insert(
                ShortCStr::from_vec(name.to_vec()).unwrap(),
                Task {
                    pidfd,
                    capture_fd: None,
                    child_pid: ret,
                    captures: Vec::new(),
                },
            );
            ret
        }
    }
}

fn pid_arg(pid: sys::Pid) -> ShortCStr {
    ShortCStr::from_vec(format!("{}", pid.as_raw()).into_bytes()).unwrap()
}

#[test]
fn posix_wait_no_tasks_returns_zero() {
    let mut state = ShellState::new();
    let status = posix_wait(&[], &mut state).unwrap();
    assert!(matches!(status, WaitStatus::Exited(0)));
}

#[test]
fn posix_wait_by_pid_reaps_child() {
    let mut state = ShellState::new();
    let pid = spawn_task(&mut state, b"bg");
    let status = posix_wait(&[pid_arg(pid)], &mut state).unwrap();
    assert!(matches!(status, WaitStatus::Exited(0)));
    assert!(state.tasks.is_empty());
}

#[test]
fn posix_wait_no_arg_reaps_all() {
    let mut state = ShellState::new();
    spawn_task(&mut state, b"one");
    spawn_task(&mut state, b"two");
    let status = posix_wait(&[], &mut state).unwrap();
    assert!(matches!(status, WaitStatus::Exited(0)));
    assert!(state.tasks.is_empty());
}

#[test]
fn posix_wait_unknown_pid_not_found() {
    let mut state = ShellState::new();
    let err = posix_wait(&[ShortCStr::from(c"999999")], &mut state)
        .err()
        .unwrap();
    assert!(matches!(err.current_context(), TaskError::NotFound));
}

#[test]
fn posix_wait_non_numeric_bad_pid() {
    let mut state = ShellState::new();
    let err = posix_wait(&[ShortCStr::from(c"abc")], &mut state)
        .err()
        .unwrap();
    assert!(matches!(err.current_context(), TaskError::BadPid { .. }));
}

#[test]
fn posix_wait_by_pid_reaps_only_named() {
    let mut state = ShellState::new();
    let p1 = spawn_task(&mut state, b"one");
    let p2 = spawn_task(&mut state, b"two");
    assert_ne!(p1, p2);
    posix_wait(&[pid_arg(p1)], &mut state).unwrap();
    // Only the named task is reaped; the other remains.
    assert_eq!(state.tasks.len(), 1);
    assert!(state.tasks.contains_key(&ShortCStr::from(c"two")));
    // Clean up the remaining task.
    let _ = posix_wait(&[], &mut state).unwrap();
}
