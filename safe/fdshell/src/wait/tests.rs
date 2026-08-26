#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
use alloc::vec;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::{FdVar, ShellState};
use crate::task::Task;
use error_stack::Report;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;
use sys::{Origin, Position, ScriptText, ShortCStr};

fn st(b: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(b.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

fn run_script(
    b: &[u8],
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    crate::script::run_script(&st(b), cell)
}

/// Run `f` in a forked child (the `wait` engine forks arm children, which must
/// not touch the test harness). The child exits 0 on success or a sentinel on a
/// panic; the parent waits and re-panics if the child fails.
fn in_child(f: impl FnOnce()) {
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_ok();
            sys::exit(if ok { 0 } else { 100 });
        }
        Some(pidfd) => {
            let status = sys::wait_pidfd::wait_pidfd(&pidfd).unwrap();
            match status {
                WaitStatus::Exited(0) => {}
                other => panic!("test child failed: {}", other.exit_code()),
            }
        }
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn insert_rd(cell: &ForkCell<ShellState>, name: &ShortCStr, fd: sys::LocalFd) {
    let mut s = cell.borrow_mut().unwrap();
    s.fds.insert(
        name.clone(),
        FdVar {
            fd,
            trace: sys::Trace::boundary(Origin::Shell),
        },
    );
}

#[test]
fn readable_pipe_arm_releases_on_nonzero_exit() {
    in_child(|| {
        let (rd, wr) = sys::pipe::pipe2(0).unwrap();
        sys::rw::write(&wr, b"hi\n").unwrap();
        let cell = make_cell();
        insert_rd(&cell, &ShortCStr::from(c"rd"), rd);
        run_script(b"wait\n readable %rd) builtin false ;;\n done", &cell).unwrap();
        let s = cell.borrow().unwrap();
        assert_eq!(s.last_status.exit_code(), 1);
        assert!(!s.fds.contains_key(&ShortCStr::from(c"rd")));
    });
}

#[test]
fn readable_pipe_arm_keeps_on_zero_exit() {
    in_child(|| {
        let (rd, wr) = sys::pipe::pipe2(0).unwrap();
        sys::rw::write(&wr, b"hi\n").unwrap();
        let cell = make_cell();
        insert_rd(&cell, &ShortCStr::from(c"rd"), rd);
        run_script(b"wait\n readable %rd) builtin true ;;\n done", &cell).unwrap();
        let s = cell.borrow().unwrap();
        assert_eq!(s.last_status.exit_code(), 0);
        assert!(s.fds.contains_key(&ShortCStr::from(c"rd")));
    });
}

#[test]
fn readable_arm_bounded_capture_appends() {
    in_child(|| {
        let (rd, wr) = sys::pipe::pipe2(0).unwrap();
        sys::rw::write(&wr, b"hi\n").unwrap();
        let cell = make_cell();
        insert_rd(&cell, &ShortCStr::from(c"rd"), rd);
        run_script(
            b"wait\n readable %rd %rd>%ready[4])\n send_fd rd %? ;;\n done",
            &cell,
        )
        .unwrap();
        let s = cell.borrow().unwrap();
        assert!(s.fds.contains_key(&ShortCStr::from(c"rd")));
        assert_eq!(s.arrays.get(&ShortCStr::from(c"ready")).unwrap().len(), 1);
    });
}

#[test]
fn after_arm_fires_when_nothing_ready() {
    in_child(|| {
        let (rd, wr) = sys::pipe::pipe2(0).unwrap();
        // `wr` stays open: %rd is neither ready nor at EOF, so poll times out.
        // The `after` arm's exit becomes ` $? `; the readable arm never runs.
        let cell = make_cell();
        insert_rd(&cell, &ShortCStr::from(c"rd"), rd);
        run_script(
            b"wait\n readable %rd) builtin true ;;\n after 5) builtin false ;;\n done",
            &cell,
        )
        .unwrap();
        drop(wr);
        let s = cell.borrow().unwrap();
        assert_eq!(s.last_status.exit_code(), 1);
        assert!(s.fds.contains_key(&ShortCStr::from(c"rd")));
    });
}

#[test]
fn ready_fd_arm_suppresses_after() {
    in_child(|| {
        let (rd, wr) = sys::pipe::pipe2(0).unwrap();
        sys::rw::write(&wr, b"hi\n").unwrap(); // %rd ready
        let cell = make_cell();
        insert_rd(&cell, &ShortCStr::from(c"rd"), rd);
        run_script(
            b"wait\n readable %rd) builtin false ;;\n after 5) builtin true ;;\n done",
            &cell,
        )
        .unwrap();
        let s = cell.borrow().unwrap();
        // The ready readable arm fires (exit 1); the `after` arm must not run.
        assert_eq!(s.last_status.exit_code(), 1);
    });
}

#[test]
fn unready_fd_arm_does_not_fire() {
    in_child(|| {
        let (a_rd, a_wr) = sys::pipe::pipe2(0).unwrap();
        sys::rw::write(&a_wr, b"hi\n").unwrap(); // %a ready
        let (b_rd, b_wr) = sys::pipe::pipe2(0).unwrap();
        // %b stays unready: `b_wr` open, no data.
        let cell = make_cell();
        insert_rd(&cell, &ShortCStr::from(c"a"), a_rd);
        insert_rd(&cell, &ShortCStr::from(c"b"), b_rd);
        run_script(
            b"wait\n readable %a) builtin true ;;\n readable %b) builtin false ;;\n done",
            &cell,
        )
        .unwrap();
        drop(b_wr);
        let s = cell.borrow().unwrap();
        // Only the ready arm (%a) fires; the unready arm (%b) is skipped, so
        // `$?` is the ready arm's 0, not the unready arm's 1.
        assert_eq!(s.last_status.exit_code(), 0);
    });
}

#[test]
fn readable_array_wildcard_fires() {
    in_child(|| {
        let (rd, wr) = sys::pipe::pipe2(0).unwrap();
        sys::rw::write(&wr, b"hi\n").unwrap(); // the array's fd is ready
        let cell = make_cell();
        {
            let mut s = cell.borrow_mut().unwrap();
            s.append_array_entry(
                &ShortCStr::from(c"arr"),
                rd,
                &ShortCStr::from(c"rd"),
                sys::Trace::boundary(Origin::Shell),
            );
        }
        // `%arr[]` resolves the whole array; a wrong `[]` strip would look up a
        // nonexistent array name and fail to build the poll set.
        run_script(b"wait\n readable %arr[]) builtin false ;;\n done", &cell).unwrap();
        let s = cell.borrow().unwrap();
        assert_eq!(s.last_status.exit_code(), 1);
    });
}

#[test]
fn finished_fd_releases_unconditionally() {
    in_child(|| {
        let (rd, wr) = sys::pipe::pipe2(0).unwrap();
        drop(wr);
        let cell = make_cell();
        insert_rd(&cell, &ShortCStr::from(c"conn"), rd);
        run_script(b"wait\n finished %conn) builtin false ;;\n done", &cell).unwrap();
        let s = cell.borrow().unwrap();
        // `finished` releases even though the arm exited 0... here it exits 1,
        // but a finished arm always releases.
        assert!(!s.fds.contains_key(&ShortCStr::from(c"conn")));
    });
}

#[test]
fn finished_task_preloads_status_and_reaps() {
    in_child(|| {
        let (child_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
        let pidfd = match pidfd_opt {
            Some(p) => p,
            None => {
                sys::exit(42);
            }
        };
        let cell = make_cell();
        {
            let mut s = cell.borrow_mut().unwrap();
            s.tasks.insert(
                c"job".into(),
                Task {
                    pidfd,
                    capture_fd: None,
                    child_pid,
                    captures: vec![],
                },
            );
        }
        run_script(
            b"wait\n finished %&job) builtin test $? -eq 42 ;;\n done",
            &cell,
        )
        .unwrap();
        let s = cell.borrow().unwrap();
        assert_eq!(s.last_status.exit_code(), 0);
        assert!(!s.tasks.contains_key(&ShortCStr::from(c"job")));
    });
}

#[test]
fn wait_block_requires_done() {
    let cell = make_cell();
    let e = run_script(b"wait\n readable %rd) builtin true ;;", &cell).unwrap_err();
    assert!(matches!(*e.current_context(), CmdError::Parse));
}

#[test]
fn wait_block_empty_errors() {
    let cell = make_cell();
    let e = run_script(b"wait\ndone", &cell).unwrap_err();
    assert!(matches!(*e.current_context(), CmdError::Parse));
}

#[test]
fn wait_unknown_pattern_errors() {
    let cell = make_cell();
    let e = run_script(b"wait\n sleeping %rd) builtin true ;;\n done", &cell).unwrap_err();
    assert!(matches!(*e.current_context(), CmdError::Parse));
}
