use crate::state::ShellState;
use error_stack::ResultExt;
use sys::fork_cell::ForkCell;

pub(crate) fn try_intercept(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, error_stack::Report<crate::error::cmd::CmdError>> {
    let cmd = cmdline
        .command
        .as_bytes()
        .change_context(crate::error::cmd::CmdError::Exec)?;
    match cmd {
        b"cd" => cd::run_cd(line, cmdline, cell),
        b"exit" | b"quit" => exit::run_exit(line, cmdline, cell),
        b"become" => become_cmd::run_become(line, cmdline, cell),
        b"export_fd" => export_fd::run_export_fd(line, cmdline, cell),
        b"wait" => wait::run_wait(line, cmdline, cell),
        b"export" => exports::run_export(line, cmdline, cell),
        b"shift" => shift::run_shift(line, cmdline, cell),
        _ => Ok(false),
    }
}

mod become_cmd;
mod cd;
mod exit;
mod export_fd;
mod exports;
mod shift;
mod validation;
mod wait;
