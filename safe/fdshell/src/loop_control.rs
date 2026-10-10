use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::state::ShellState;

/// Control-flow signals propagated up through the execution stack.
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(crate) enum LoopControl {
    /// Exit `n` enclosing loops (`break n`), `n >= 1`.
    Break(u32),
    /// Advance `n` enclosing loops to their next iteration (`continue n`), `n >= 1`.
    Continue(u32),
    /// Leave the current function (status already set by the `return` that raised it).
    Return,
    /// Leave the shell (errexit); the failing status is already in `last_status`.
    Exit,
}

/// Consume one enclosing loop level from `control`: a `Break(n)`/`Continue(n)`
/// count drops by one, and count 1 is fully consumed (`None`). `Return`/`Exit`
/// are not loop counts and propagate unchanged.
pub(crate) fn consume(control: LoopControl) -> Option<LoopControl> {
    match control {
        LoopControl::Break(1) | LoopControl::Continue(1) => None,
        LoopControl::Break(n) => Some(LoopControl::Break(n - 1)),
        LoopControl::Continue(n) => Some(LoopControl::Continue(n - 1)),
        LoopControl::Return | LoopControl::Exit => Some(control),
    }
}

/// Run `f` inside one more loop level: `state.loop_depth` is incremented before
/// `f` and restored before `f`'s result is returned, on every exit path (the
/// pairing rule of [`crate::nest::deeper`], and of the `local` frame in task #108).
pub(crate) fn in_loop<T, F>(cell: &ForkCell<ShellState>, f: F) -> Result<T, Report<CmdError>>
where
    F: FnOnce() -> Result<T, Report<CmdError>>,
{
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.loop_depth += 1;
    drop(state); // never hold the borrow across the recursive body
    let result = f();
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.loop_depth -= 1;
    result
}

#[cfg(test)]
mod tests;
