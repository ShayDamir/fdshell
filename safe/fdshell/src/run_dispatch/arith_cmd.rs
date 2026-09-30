use crate::error::cmd::CmdError;
use crate::parse::ParsedLine;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

/// Run the `((expr))` arithmetic command: evaluate `expr` in-process and set
/// the exit status to `expr == 0` (bash: 0 when the value is non-zero, 1
/// when it is zero). Returns `true` when `parsed` was an arithmetic command.
pub(super) fn run(
    parsed: &ParsedLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    let ParsedLine::ArithCommand(body) = parsed else {
        return Ok(false);
    };
    let bytes = body.as_bytes().change_context(CmdError::Resolve)?;
    let value = crate::arith::eval(bytes, cell).change_context(CmdError::Resolve)?;
    // The `eval` borrow of the cell ends here; `set_last_exit` reborrows.
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(i32::from(value == 0));
    Ok(true)
}
