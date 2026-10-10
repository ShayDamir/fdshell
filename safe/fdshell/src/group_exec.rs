//! The `{ …; }` group command (POSIX #1.2): the body runs in the current
//! shell (no fork), so its assignments, `cd`, and fd state persist past the
//! group, and the redirections written after `}` apply to the shell for the
//! duration of the group.

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::parse::group_block::GroupBlock;
use crate::state::ShellState;
use error_stack::Report;
use sys::fork_cell::ForkCell;

/// Run the group: open its redirect scope, run the body as a script one
/// nesting level deeper, and put the shell's fds back when it finishes.
pub(crate) fn run_group(
    group: &GroupBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    // The scope opens before the body, so a failing redirection open aborts the
    // group without running it (the same rule as a function call, `run/parent.rs`).
    let scope = crate::redirect::Scope::open(&group.redirects, cell)?;
    let result = crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
        crate::script::run_script(&group.body, cell)
    });
    match result {
        Ok(control) => {
            scope.restore()?;
            Ok(control)
        }
        // The body's report is the actionable error; the restore is
        // best-effort, the same rule as `run/parent.rs`.
        Err(report) => {
            let _ = scope.restore();
            Err(report)
        }
    }
}

#[cfg(test)]
mod tests;
