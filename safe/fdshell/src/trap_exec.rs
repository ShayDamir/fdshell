// mutants: -C --lib -C --test -C trap -- -E 'test(/^trap_exec::/) | binary(trap)'

//! The deferred executor for traps: the `sys` handler only flags (task #582),
//! the `ShellState` trap table only stores (task #583), the `trap` builtin only
//! registers (task #584) — this module *runs* a stored action at a safe point,
//! the dash `trap.c::dotrap` / bash `trap.c::run_pending_traps` pattern. It is
//! the only place allowed to run the runner for a pending trap, and the exit
//! paths call [`run_exit_trap`] because the `EXIT` slot is never in the pending
//! mask (`sigaction` answers `EINVAL` for `0`).

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};
use error_stack::{Report, ResultExt, bail};
use sys::fork_cell::ForkCell;
use sys::{Origin, Position, ScriptText, ShortCStr};

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::{EXIT, ShellState, TrapDisposition};

/// The EXIT/0 trap runs at most once per shell (bash 5.3.9, measured: an
/// `exit` inside a running action exits with its status without re-entering
/// the trap), and the guard is a `static` because the trap bookkeeping is
/// process-global already, like the pending mask and the dispositions (LESSONS
/// "A real signal handler relies on glibc's `sigaction` to fill the restorer
/// (task #582)").
static EXIT_TRAP_RAN: AtomicBool = AtomicBool::new(false);

/// Check-and-run at a command boundary (dash's `dotrap` / bash's
/// `run_pending_traps`: the handler only flags, execution happens here).
/// Consumes the pending mask — one lock-free swap, so the fast path when
/// nothing fired is one atomic read plus a zero check — and runs each pending
/// action's bytes as a command line in the current environment, in ascending
/// condition order (the store's order property, task #583/#584). An `Ignore`
/// entry needs no execution: the disposition itself did the work; `Default`
/// cannot be pending (nothing is `sigaction`ed for it).
pub(crate) fn run_pending(
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let mask = sys::signal::take_pending().change_context(CmdError::Never)?;
    let mut raised = None;
    for cond in 1..=64i32 {
        if mask & (1u64 << (cond - 1)) == 0 {
            continue;
        }
        let Some(body) = action_body(cond, cell)? else {
            continue;
        };
        match run_action(&body, cell) {
            Ok(control) => raised = raised.or(control),
            Err(report) => report_and_continue(report),
        }
    }
    Ok(raised)
}

/// Run the EXIT/0 trap at a shell exit path, at most once per shell, with the
/// same `$?` rules as [`run_action`]. An `exit` raised inside a running action
/// exits with that status without re-entering the trap ([`EXIT_TRAP_RAN`]).
pub(crate) fn run_exit_trap(cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    if EXIT_TRAP_RAN.swap(true, Ordering::Relaxed) {
        return Ok(());
    }
    let Some(body) = action_body(EXIT, cell)? else {
        return Ok(());
    };
    match run_action(&body, cell) {
        Ok(_) => {}
        Err(report) => report_and_continue(report),
    }
    Ok(())
}

/// Run one stored action as a full command line (the list form's re-input
/// contract, task #584), so the runner is `run_script` and the nesting cap
/// counts the trap level — `eval_cmd`'s shape.
///
/// `$?`: the action **sees** the status the shell held when it was invoked, and
/// the value is **restored** after it (POSIX, Austin Group #1602, bash
/// measured) — an action must not be able to change what the shell sees.
/// Restoring the *code* rather than the whole `WaitStatus` is exact for every
/// consumer (`$?` renders `exit_code()`, and nothing matches `last_status` on
/// its variant) and keeps `siginfo.rs` untouched.
fn run_action(
    body: &ShortCStr,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let saved = {
        let state = cell.borrow().change_context(CmdError::Never)?;
        state.last_status.exit_code()
    };
    let text = ScriptText::new(body.clone(), Position::new(1, 1), Origin::Shell);
    let result = crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
        crate::script::run_script(&text, cell)
    });
    {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_last_exit(saved);
    }
    result
}

/// The action bytes of `cond`, cloned **out** of the borrow and dropping it
/// before any run (LESSONS "Never hold a ForkCell borrow across a recursive
/// call that re-borrows"). Every other disposition — `Default`, and the
/// `Ignore` whose empty action needs no execution — answers `None`.
fn action_body(
    cond: i32,
    cell: &ForkCell<ShellState>,
) -> Result<Option<ShortCStr>, Report<CmdError>> {
    let Ok(state) = cell.borrow() else {
        bail!(CmdError::Never);
    };
    Ok(match state.trap_at(cond) {
        TrapDisposition::Action(body) => Some(body),
        _ => None,
    })
}

/// Report and continue: a failing action is reported and the shell carries on
/// (bash measured) — the `trap_cmd::report_and_continue` precedent.
fn report_and_continue(report: Report<CmdError>) {
    let _ = writeln!(crate::io::Stderr, "{report:?}");
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
