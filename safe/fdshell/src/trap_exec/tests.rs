use super::*;
use sys::SyscallError;
use sys::errno;
use sys::pipe::pipe2;
use sys::siginfo::WaitStatus;
use sys::signal::Action;
use sys::signalfd::SIGUSR1;

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

/// Fork a child with `ForkCell::fork_pidfd`, let it report one `usize` over a
/// pipe, and return what the parent read.
///
/// Install, deliver and run all run **in the child**: the in-process harness
/// runtime of this `no_std` test binary never runs a `sigaction`ed handler,
/// while its forked children do (measured, task #585) — so the signal rows
/// measure inside the child, which is also what keeps the process-global
/// disposition and the pending mask row-local. Nothing panics in the child (a
/// forked test harness catches a panic and the child would still exit 0 —
/// LESSONS), so a failed child-side check travels as exit 1 and the parent's
/// reap assertion reports it. The child allocates nothing: the harness's pipe
/// and `ForkCell` are built before the fork, and the 128 MB virtual-address
/// cap is respected.
fn child_reports(cell: &ForkCell<ShellState>, report: impl FnOnce() -> usize) -> usize {
    let (rd, wr) = pipe2(0).unwrap();
    let (ret, pidfd_opt) = cell.fork_pidfd().unwrap();
    if ret.as_raw() == 0 {
        let value = report();
        if wr.write_all(&value.to_ne_bytes()).is_err() {
            sys::exit(1);
        }
        sys::exit(0);
    }

    // Parent: drop the write end, so the read returns 0 if the child dies
    // before writing anything instead of blocking forever.
    drop(wr);
    let mut buf = [0u8; 8];
    let n = rd.read(&mut buf).unwrap();

    let pidfd = pidfd_opt
        .ok_or(SyscallError::Other {
            errno: errno::EINVAL,
            syscall: "trap_exec_child",
        })
        .unwrap();
    match pidfd.wait_pidfd().unwrap() {
        WaitStatus::Exited(0) => {}
        other => {
            // The child's report failed (a panic it swallowed, a failed check,
            // or a write error): the parent's panic reports the status; the
            // bytes read are meaningless then.
            panic!("child exited with status {}", other.exit_code());
        }
    }
    assert_eq!(n, 8, "child reported {n} of 8 bytes");
    usize::from_ne_bytes(buf)
}

/// The action runs through the **in-process** runner, so an assignment inside
/// it lands in the shell's own store (bash measured: the action sees and
/// mutates shell state), and `$?` is restored to what the shell held when the
/// action was invoked even though the action ended on `false` (status 1).
///
/// Install, deliver and run all run in the forked child (see
/// [`child_reports`]); the cell's trap table and `$?` are set by the parent
/// before the fork so the child's report measures the inherited state. The
/// reported number is the three checks folded: the action ran, `$?` restored,
/// no loop control raised.
#[test]
fn the_action_runs_in_the_current_environment() {
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.set_trap(
            SIGUSR1,
            TrapDisposition::Action(ShortCStr::from(c"X=set; false")),
        );
        state.set_last_exit(3);
    }
    let report = child_reports(&cell, || {
        sys::signal::set_action(SIGUSR1, Action::Record).unwrap();
        let _ = sys::signal::kill(sys::env::getpid(), SIGUSR1);
        // One syscall boundary makes the delivery definite (task #582's rule).
        let _ = sys::env::getpid();
        let control = run_pending(&cell).unwrap();
        let state = cell.borrow().unwrap();
        let ran = state
            .strings
            .get(&ShortCStr::from(c"X"))
            .is_some_and(|v| v.value.as_bytes().unwrap() == b"set");
        let restored = state.last_status.exit_code() == 3;
        usize::from(ran && restored && control.is_none())
    });
    assert_eq!(report, 1, "the action ran here, $? was restored");
}

/// Nothing fired: the fast path is the swap plus a zero check, it runs nothing
/// and leaves `$?` alone. This row owns no disposition and delivers nothing —
/// nextest gives each test its own process, so the mask is certainly its own,
/// empty one.
#[test]
fn the_empty_mask_fast_path_runs_nothing() {
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.set_last_exit(7);
    }
    assert_eq!(run_pending(&cell).unwrap(), None);
    let state = cell.borrow().unwrap();
    assert_eq!(
        state.last_status.exit_code(),
        7,
        "the fast path leaves $? alone"
    );
    assert!(
        state.strings.get(&ShortCStr::from(c"QQZABSENT")).is_none(),
        "nothing ran"
    );
}

/// An empty action **is** the ignore disposition (task #583's normalization),
/// and the disposition does the work itself: a condition registered as
/// `Ignore` consumes its pending bit and runs nothing. The row runs in the
/// forked child (see [`child_reports`]) and reports the three checks folded:
/// the bit was consumed without running, `$?` unchanged, no loop control.
#[test]
fn an_ignore_entry_needs_no_execution() {
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.set_trap(SIGUSR1, TrapDisposition::Ignore);
        state.set_last_exit(4);
    }
    let report = child_reports(&cell, || {
        sys::signal::set_action(SIGUSR1, Action::Record).unwrap();
        let _ = sys::signal::kill(sys::env::getpid(), SIGUSR1);
        let _ = sys::env::getpid();
        let control = run_pending(&cell).unwrap();
        let state = cell.borrow().unwrap();
        let untouched = state.last_status.exit_code() == 4;
        let nothing_ran = state.strings.get(&ShortCStr::from(c"QQZABSENT")).is_none();
        usize::from(untouched && nothing_ran && control.is_none())
    });
    assert_eq!(report, 1, "an `Ignore` consumes the bit and runs nothing");
}

/// The EXIT slot runs at the exit paths, and **at most once per shell**: the
/// second call must leave the already-rewritten variable exactly as the first
/// run of the action left it (bash 5.3.9, measured: an `exit` inside a running
/// action exits with its status without re-entering the trap). The action
/// rewrites `$X` with a suffix, so a re-run would double it. No signal is
/// involved — the EXIT slot is the table key `0`, filled by the parent
/// directly — so the row stays in-process like the fast-path one.
#[test]
fn the_exit_trap_runs_at_most_once_per_shell() {
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.set_trap(EXIT, TrapDisposition::Action(ShortCStr::from(c"X=${X}z")));
    }
    run_exit_trap(&cell).unwrap();
    {
        let state = cell.borrow().unwrap();
        let x = state.strings.get(&ShortCStr::from(c"X")).unwrap();
        assert_eq!(
            x.value.as_bytes().unwrap(),
            b"z",
            "the first call ran the action"
        );
    }
    run_exit_trap(&cell).unwrap();
    let state = cell.borrow().unwrap();
    let x = state.strings.get(&ShortCStr::from(c"X")).unwrap();
    assert_eq!(
        x.value.as_bytes().unwrap(),
        b"z",
        "the guard makes the second call a no-op"
    );
}
