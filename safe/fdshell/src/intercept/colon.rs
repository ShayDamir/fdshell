use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

/// `:` — the null utility: does nothing, returns 0. Accepts (and discards)
/// arguments and redirects so the `: "${var:=d}"` and `: <<EOF` idioms work;
/// redirects are applied by `redirect::Scope` in `run/parent.rs`, so `: > file`
/// creates the file. Captures stay rejected, as for every intercept.
pub(crate) fn run_colon(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::check_captures_not_supported(line, ":", &cmdline.captures)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(0);
    Ok(true)
}

#[cfg(test)]
mod tests;
