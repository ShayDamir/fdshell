use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

/// `:` — the null utility: does nothing, returns 0. Accepts (and discards)
/// arguments and redirects so the `: "${var:=d}"` and `: <<EOF` idioms work.
pub(crate) fn run_colon(
    _line: &[u8],
    _cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(0);
    Ok(true)
}

#[cfg(test)]
mod tests;
