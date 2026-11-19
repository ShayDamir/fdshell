// mutants: -C --lib -C --test -C trap -- -E 'binary(trap)'

use error_stack::{Report, ResultExt, ensure};

use crate::error::cmd::CmdError;
use crate::numarg::{LEGAL_NUMBER, decimal};
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_exit(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "exit", cmdline)?;

    let code = match cmdline.args.first() {
        Some(arg) => {
            let bytes = arg.as_bytes().change_context(CmdError::Never)?;
            decimal(bytes, &LEGAL_NUMBER).ok_or(CmdError::InvalidArgument { arg: "exit code" })?
        }
        None => {
            let state = cell.borrow().change_context(CmdError::Never)?;
            state.last_status.exit_code() as i128
        }
    };
    ensure!((0..=255).contains(&code), CmdError::ExitArgInvalid);
    let code = i32::try_from(code).change_context(CmdError::Never)?;
    // The EXIT trap runs at most once per shell before the process ends
    // (bash measured), so an `exit` raised from inside a trap action exits
    // with its status without re-entering the trap.
    crate::trap_exec::run_exit_trap(cell)?;
    sys::exit(code);
}
