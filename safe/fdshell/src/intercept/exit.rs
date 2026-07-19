use error_stack::{Report, ResultExt, ensure};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_exit(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "exit", cmdline)?;

    let code = match cmdline.args.first() {
        Some(arg) => arg
            .parse::<i32>()
            .change_context(CmdError::InvalidArgument { arg: "exit code" })?,
        None => {
            let state = cell.borrow().change_context(CmdError::Never)?;
            state.last_status.exit_code()
        }
    };
    ensure!((0..=255).contains(&code), CmdError::ExitArgInvalid);
    sys::exit(code);
}
