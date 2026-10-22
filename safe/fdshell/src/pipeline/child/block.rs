//! The keyword-block stage's child: bash runs **every** pipeline stage in its
//! own subshell, so the block runs here, in the forked child, through the
//! identical foreground path (`run::run_parsed`) — the shape
//! `run/async_job.rs::run_and_exit` uses for a backgrounded statement. The
//! fork, the fd 0/1 pipe ends and the `close_inherited` cleanup are already the
//! pipeline child's own, so a block stage is a builtin-shaped stage: it never
//! execs, it re-enters the interpreter (task #292).
//!
//! Running the block in the child (not in the parent) is what makes `cd`,
//! assignments, `exit` and `break` written in the stage die with the child:
//! `for i in 1; do echo a | while :; do break; done; echo tail; done` prints
//! `tail`, and `echo a | while :; do exit 3; done; echo tail rc=$?` prints
//! `tail rc=3` with the parent alive (both measured against bash 5.3.9).

use crate::child::apply_redirects;
use crate::error::child_process::ChildProcessError;
use crate::parse::BlockStage;
use crate::redirect::Redirect;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

/// Run the block stage in this child and report the code the child exits with:
/// the block's own last status. A loop control (`break`/`continue`/`exit`) that
/// escapes the stage's own loops leaves through the same `Ok` arm — it dies with
/// the child, it never reaches the parent's loop. An error of the block's own
/// (`'break' is not inside a loop`, a failed redirect) is the caller's report
/// arm: `pipeline::launch_pipeline` prints it and exits with its code.
pub(super) fn run_block(
    block: &BlockStage,
    redirects: &[Redirect],
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<ChildProcessError>> {
    // The fd 0/1 pipe ends are what makes `printf 'x\n' | while read l; do
    // echo "$l"; done` read the pipe as the loop's stdin.
    apply_redirects(redirects)?;
    // No cell borrow is held across the interpreter re-entry (LESSONS: a held
    // `RefMut` poisons the child's and the parent's nested borrows).
    crate::run::run_parsed(&block.parsed, &block.text, cell, false)
        .map(|_| last_status_code(cell))
        .change_context(ChildProcessError::StageFailed)
}

/// The child's own last status, the code `sys::exit` carries
/// (`async_job::last_status_code` is the model). An un-borrowable state is an
/// invariant breach with no child-side report channel: status 1.
fn last_status_code(cell: &ForkCell<ShellState>) -> i32 {
    cell.borrow()
        .map(|state| state.last_status.exit_code())
        .unwrap_or(1)
}
