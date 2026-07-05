use error_stack::{Report, ResultExt, ensure};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

use super::validation::*;

pub(crate) fn run_exit(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    check_builtin_not_supported(line, "exit", cmdline.builtin)?;
    check_captures_not_supported(line, "exit", &cmdline.captures)?;
    check_redirects_not_supported(line, "exit", &cmdline.redirects)?;

    let code = match cmdline.args.first() {
        Some(arg) => {
            let s = core::str::from_utf8(arg.as_bytes().change_context(CmdError::Exec)?)
                .change_context(CmdError::ExitArgInvalid)?;
            s.parse::<i32>().change_context(CmdError::ExitArgInvalid)?
        }
        None => {
            let state = cell.borrow().change_context(CmdError::Exec)?;
            state.last_status.exit_code()
        }
    };
    ensure!((0..=255).contains(&code), CmdError::ExitArgInvalid);
    std::process::exit(code);
}
