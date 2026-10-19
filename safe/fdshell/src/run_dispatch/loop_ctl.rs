use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use error_stack::{Report, ResultExt, ensure};
use sys::fork_cell::ForkCell;

/// What raising a loop control leaves behind: the clamped signal, or the bail.
type Raised = Result<Option<LoopControl>, Report<CmdError>>;

/// Raise a `break`/`continue` at the loop depth this site has reached: `n` is
/// clamped to it (LESSONS "Clamp a `break`/`continue` count at the call site"),
/// depth 0 bails with `outside`, and the signal's own run succeeded, so it
/// leaves `$?` = 0 exactly as bash's `break`/`continue` builtins do (task #262).
/// `control` is `LoopControl::Break` or `Continue`.
fn raise(
    n: &u32,
    cell: &ForkCell<ShellState>,
    outside: CmdError,
    control: fn(u32) -> LoopControl,
) -> Raised {
    cell.borrow_mut()
        .change_context(CmdError::Never)
        .and_then(|mut state| {
            ensure!(state.loop_depth > 0, outside);
            state.set_last_exit(0);
            Ok(control(*n.min(&state.loop_depth)))
        })
        .map(Some)
}

/// `break`: the clamped signal, and the 0 its run leaves behind.
pub(super) fn raise_break(n: &u32, cell: &ForkCell<ShellState>) -> Raised {
    raise(n, cell, CmdError::BreakOutsideLoop, LoopControl::Break)
}

/// `continue`: the clamped signal, and the 0 its run leaves behind.
pub(super) fn raise_continue(n: &u32, cell: &ForkCell<ShellState>) -> Raised {
    raise(
        n,
        cell,
        CmdError::ContinueOutsideLoop,
        LoopControl::Continue,
    )
}
