//! The `time` reserved word: run a statement, then report its real/user/sys
//! times on stderr (POSIX XCU 2.4 reserved words + the `time` utility page).

pub(crate) mod report;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::parse::{ParsedLine, TimeBlock};
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

/// The commands that end the shell process. Their report is printed *before*
/// they run, because nothing runs after them: `time exit 3` reports, as bash
/// does. `.`/`source` is not one of them — it runs the script in the *current*
/// shell and returns, so its report prints after the sourced script, like every
/// statement that returns.
const TERMINATING: &[&[u8]] = &[b"exit", b"quit", b"exec", b"become"];

/// Run the `time` statement: snapshot, run the inner statement, report the
/// deltas, and return the inner's result (its status and its control flow are
/// the statement's own; the report never swallows them).
///
/// The report is emitted on the error path too (best effort, the
/// `group_exec` restore rule), so `time cd /nope` prints it like bash does.
pub(crate) fn run_time(
    block: &TimeBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let start = report::start(cell)?;
    if terminates_shell(block) {
        report::emit(block.posix, &start, cell, Ok(None)).ok();
        return run_inner(block, cell);
    }
    report::emit(block.posix, &start, cell, run_inner(block, cell))
}

/// The timed statement ends the shell process: an `exit`/`exec` family command
/// on its own. A pipeline (`exit | cat`) is not one — its `exit` runs in a
/// child, and the pipeline's own report prints normally.
fn terminates_shell(block: &TimeBlock) -> bool {
    let Some(ParsedLine::Cmd(cmd)) = block.inner.as_deref() else {
        return false;
    };
    cmd.command
        .as_bytes()
        .is_ok_and(|name| TERMINATING.contains(&name))
}

/// Run the timed statement; a bare `time` has no inner and succeeds with 0.
fn run_inner(
    block: &TimeBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    match &block.inner {
        Some(inner) => crate::run::run_parsed(inner, &block.inner_text, cell, false),
        None => {
            cell.borrow_mut()
                .change_context(CmdError::Never)?
                .set_last_exit(0);
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests;
