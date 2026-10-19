#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::parse::parse;
use sys::fork_cell::ForkCell;
use sys::{Origin, Position, ShortCStr};

fn cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn st(b: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(b.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// Spawn `b` as a nameless job and reap every job the parent recorded,
/// returning the reaped statuses (POSIX `wait` with no operands reaps the
/// named table and the nameless deque; the child is never left running).
fn spawn_and_reap(b: &[u8]) -> Vec<sys::siginfo::WaitStatus> {
    let cell = cell();
    let text = st(b);
    let parsed = parse(&text).unwrap();
    let mut statuses = Vec::new();
    spawn(&parsed, &text, &cell).unwrap();
    let mut state = cell.borrow_mut().unwrap();
    while !state.jobs.is_empty() {
        statuses.push(crate::task::posix_wait(&[], &mut state).unwrap());
    }
    statuses
}

fn exited(st: sys::siginfo::WaitStatus) -> bool {
    matches!(st, sys::siginfo::WaitStatus::Exited(0))
}

/// The parent side: a forked child is recorded as a nameless job (`$!` reports
/// it, `wait` reaps it), the parent's status is 0 and it continues — no
/// `LoopControl`, no foreground execution.
#[test]
fn spawn_records_a_nameless_job_and_continues() {
    let cell = cell();
    let text = st(b"builtin echo hi");
    let parsed = parse(&text).unwrap();
    let control = spawn(&parsed, &text, &cell).unwrap();
    assert!(control.is_none());
    let state = cell.borrow().unwrap();
    assert_eq!(state.jobs.anon.len(), 1);
    assert_eq!(state.last_bg_pid, state.jobs.anon[0].child_pid.into());
    assert!(matches!(
        state.last_status,
        sys::siginfo::WaitStatus::Exited(0)
    ));
    drop(state);
    // Hermeticity: the child is reaped in-test.
    let mut state = cell.borrow_mut().unwrap();
    crate::task::posix_wait(&[], &mut state).unwrap();
    assert!(state.jobs.is_empty());
}

/// The child runs the statement through the identical foreground path and
/// `_exit`s with **its** status: a failing statement exits 1, where the
/// parent's own status stayed 0 (the `&` job's status is only visible through
/// `wait <pid>`).
#[test]
fn child_exits_with_the_status_of_its_own_statement() {
    // POSIX: `wait` with no operands reports 0 whatever the child exited with,
    // so the child's own status is read through the pid form.
    for script in [b"builtin false".as_slice(), b"builtin true".as_slice()] {
        let cell = cell();
        let text = st(script);
        let parsed = parse(&text).unwrap();
        spawn(&parsed, &text, &cell).unwrap();
        let mut state = cell.borrow_mut().unwrap();
        let pid = state.jobs.anon[0].child_pid;
        let arg = ShortCStr::from_vec(alloc::format!("{}", pid.as_raw()).into_bytes()).unwrap();
        let status = crate::task::posix_wait(&[arg], &mut state).unwrap();
        let code = status.exit_code();
        let want = if script == b"builtin false" { 1 } else { 0 };
        assert_eq!(code, want, "{script:?}");
    }
}

/// A child whose statement fails to launch prints its report and exits 1
/// (the shell's convention for a command error): the parent's record is a
/// job whose pidfd reports 1.
#[test]
fn child_error_path_exits_one() {
    let cell = cell();
    let text = st(b"fdshell_async_missing_command");
    let parsed = parse(&text).unwrap();
    spawn(&parsed, &text, &cell).unwrap();
    let mut state = cell.borrow_mut().unwrap();
    let pid = state.jobs.anon[0].child_pid;
    let arg = ShortCStr::from_vec(alloc::format!("{}", pid.as_raw()).into_bytes()).unwrap();
    let status = crate::task::posix_wait(&[arg], &mut state).unwrap();
    assert_eq!(status.exit_code(), 1);
}

/// A loop control written in background (`exit &`, `break &`) dies with the
/// child: the parent sees no `LoopControl` and keeps running.
#[test]
fn loop_control_dies_with_the_child() {
    let cell = cell();
    let text = st(b"exit");
    let parsed = parse(&text).unwrap();
    let control = spawn(&parsed, &text, &cell).unwrap();
    assert!(control.is_none());
    let mut state = cell.borrow_mut().unwrap();
    assert_eq!(state.jobs.anon.len(), 1);
    crate::task::posix_wait(&[], &mut state).unwrap();
}

/// The child's copy of `ShellState` is discarded: a statement that mutates
/// the shell (`cd`, an assignment run through the `Cmd` path) never reaches
/// the parent, which is what keeps `cd /tmp &` bash-parity.
#[test]
fn child_state_never_reaches_the_parent() {
    let before = std::env::current_dir().unwrap();
    let cell = cell();
    let text = st(b"cd /");
    let parsed = parse(&text).unwrap();
    spawn(&parsed, &text, &cell).unwrap();
    let mut state = cell.borrow_mut().unwrap();
    crate::task::posix_wait(&[], &mut state).unwrap();
    // The parent's own cwd is untouched: the child's `cd` died with the child.
    assert_eq!(std::env::current_dir().unwrap(), before);
}
