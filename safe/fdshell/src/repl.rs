use error_stack::Report;

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) use crate::cond::run_cond_list;
pub(crate) use crate::script::run_script;

pub fn handle(line: &[u8], cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    run_script(line, cell)?;
    Ok(())
}

pub fn exec_cmd(line: &[u8], cell: &ForkCell<ShellState>) -> Result<i32, Report<CmdError>> {
    run_script(line, cell)
}
