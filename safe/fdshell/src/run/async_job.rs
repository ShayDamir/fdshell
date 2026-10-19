//! The plain `&` async job (task #326): fork the whole shell so the child
//! runs the statement through the **identical foreground path** (word-0
//! expansion, function lookup, parent-side intercepts, pipeline launch with
//! its internal reaping) inside its copy of `ShellState`, then exits. The
//! parent records a nameless job and continues with status 0 — bash's
//! background shape, and the discarded child is what makes `cd x &` and
//! `exit &` leave the parent's state untouched.

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::parse::ParsedLine;
use crate::state::ShellState;
use alloc::vec::Vec;
use core::fmt::Write;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// Run `parsed` asynchronously: fork the shell ( [`sys::fork_pidfd_cell`], the
/// single fork choke point), let the child run the foreground path and
/// `sys::exit` with its status, and record the child as a nameless job in the
/// parent (`$!` reports it, `wait`/`wait <pid>` reap it).
/// A `LoopControl` (`break`/`continue`/`exit` written in background)
/// terminates the child — it never reaches the parent.
pub(crate) fn spawn(
    parsed: &ParsedLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    // No cell borrow is held across the fork (LESSONS: a held `RefMut`
    // poisons the child's and the parent's nested borrows).
    let (child_pid, pidfd) =
        sys::fork_pidfd::fork_pidfd_cell(cell).change_context(CmdError::Launch)?;
    let Some(pidfd) = pidfd else {
        run_and_exit(parsed, text, cell);
    };
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.jobs.push_anon(crate::task::Task {
        pidfd,
        capture_fd: None,
        child_pid,
        captures: Vec::new(),
    });
    state.last_bg_pid = Some(child_pid);
    state.last_status = sys::siginfo::WaitStatus::Exited(0);
    Ok(None)
}

/// The child side: the full foreground path, then `_exit`. An error prints
/// its report like `child::child_main` does and exits 1 (the shell convention
/// for a command error whose status the child itself determines); a loop
/// control raised in the child dies with the child, carrying the status it
/// had reached.
fn run_and_exit(parsed: &ParsedLine, text: &ScriptText, cell: &ForkCell<ShellState>) -> ! {
    match crate::run::run_parsed(parsed, text, cell, false) {
        Ok(_) => sys::exit(last_status_code(cell)),
        Err(report) => {
            let _ = writeln!(crate::io::Stderr, "{report:?}");
            sys::exit(1);
        }
    }
}

/// The child's own last status, the code `sys::exit` carries. An un-borrowable
/// state is an invariant breach with no child-side report channel: exit 1.
fn last_status_code(cell: &ForkCell<ShellState>) -> i32 {
    cell.borrow()
        .map(|state| state.last_status.exit_code())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests;
