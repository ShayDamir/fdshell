//! Cell-level frame push/pop for `function_call` (narrow borrows, LESSONS:
//! never hold a `ForkCell` borrow across a recursive call). The module `frames`
//! keeps the frame itself and its `ShellState` methods; the pair here is the
//! only part that reaches the state through the cell.

use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::state::ShellState;

/// Enter a frame through the cell, so `local` in the call has one to write into.
pub fn push_frame(cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.begin_frame();
    Ok(())
}

/// Leave the innermost frame through the cell, putting every shadowed value back.
pub fn pop_frame(cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.end_frame();
    Ok(())
}
