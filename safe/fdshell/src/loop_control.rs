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

/// What one loop level does with a control raised inside it — its body **or**
/// its condition. `Break(1)`/`Continue(1)` are consumed by this level, a larger
/// count hands the remainder to the enclosing level, and `Return`/`Exit` are not
/// loop counts, so they pass through unchanged.
pub(crate) enum AtLevel {
    /// Stop this loop (`break 1`).
    Stop,
    /// Advance this loop to its next iteration (`continue 1`).
    Next,
    /// The signal belongs to an enclosing level.
    Outer(LoopControl),
}

/// Map a control raised inside one of this loop's own levels to [`AtLevel`].
pub(crate) fn at_level(control: LoopControl) -> AtLevel {
    match control {
        LoopControl::Break(1) => AtLevel::Stop,
        LoopControl::Continue(1) => AtLevel::Next,
        LoopControl::Break(n) => AtLevel::Outer(LoopControl::Break(n - 1)),
        LoopControl::Continue(n) => AtLevel::Outer(LoopControl::Continue(n - 1)),
        LoopControl::Return | LoopControl::Exit => AtLevel::Outer(control),
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
