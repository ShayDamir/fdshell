use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

pub(crate) fn run_shift(
    _line: &[u8],
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let n = match cmdline.args.first() {
        None => 1,
        Some(arg) => {
            let bytes = arg.as_bytes().change_context(CmdError::Never)?;
            let s = core::str::from_utf8(bytes)
                .change_context(CmdError::InvalidArgument { arg: "shift count" })?;
            s.parse::<usize>()
                .change_context(CmdError::InvalidArgument { arg: "shift count" })?
        }
    };
    state.shift(n);
    state.set_last_exit(0);
    Ok(true)
}
