#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;
use sys::{Origin, Position, ScriptText, ShortCStr};

use super::Kind;
use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::{FdVar, ShellState};
use error_stack::Report;

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
/// not touch the test harness). The child exits a sentinel on a panic.
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

/// Pin the exact event bits each arm kind polls for and treats as "ready".
#[test]
fn events_bits_per_kind() {
    use sys::poll::*;
    assert_eq!(
        super::events(Kind::Readable, false),
        (POLLIN, POLLIN + POLLERR + POLLHUP + POLLNVAL)
    );
    assert_eq!(
        super::events(Kind::Writable, false),
        (POLLOUT, POLLOUT + POLLERR + POLLHUP + POLLNVAL)
    );
    assert_eq!(
        super::events(Kind::Finished, false),
        (POLLRDHUP + POLLHUP, POLLRDHUP + POLLHUP)
    );
    assert_eq!(super::events(Kind::Finished, true), (POLLIN, POLLIN));
}

/// With no `after` arm the deadline is block-until-ready, not poll-once. A
/// background writer posts data after a delay, so a timeout-0 mutation would
/// poll once, see nothing, and skip the arm (leaving last_status at 0).
#[test]
fn readable_arm_waits_for_late_data() {
    in_child(|| {
        let (rd, wr) = sys::pipe::pipe2(0).unwrap();
        let (_, helper_pidfd) = sys::fork_pidfd::fork_pidfd().unwrap();
        match helper_pidfd {
            None => {
                std::thread::sleep(std::time::Duration::from_millis(50));
                let _ = sys::rw::write(&wr, b"hi\n");
                sys::exit(0);
            }
            Some(pidfd) => {
                let cell = make_cell();
                insert_rd(&cell, &ShortCStr::from(c"rd"), rd);
                run_script(b"wait\n readable %rd) builtin false ;;\n done", &cell).unwrap();
                let s = cell.borrow().unwrap();
                assert_eq!(s.last_status.exit_code(), 1);
                let _ = sys::wait_pidfd::wait_pidfd(&pidfd).unwrap();
            }
        }
    });
}
