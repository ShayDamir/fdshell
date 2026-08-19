use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

pub(crate) fn try_intercept(
    text: &ScriptText,
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    let line = text.as_bytes().change_context(CmdError::Never)?;
    let cmd = cmdline.command.as_bytes().change_context(CmdError::Never)?;
    match cmd {
        b"cd" => cd::run_cd(line, cmdline, cell),
        b"exit" | b"quit" => exit::run_exit(line, cmdline, cell),
        b"become" => become_cmd::run_become(line, cmdline, cell),
        b"exec" => become_cmd::run_exec(line, cmdline, cell),
        b"export_fd" => export_fd::run_export_fd(line, cmdline, cell),
        b"wait" => wait::run_wait(line, cmdline, cell),
        b"export" => exports::run_export(line, cmdline, text, cell),
        b"envfilter" => envfilter::run_envfilter(line, cmdline, cell),
        b"shift" => shift::run_shift(line, cmdline, cell),
        b"read" => read::run_read(line, cmdline, text, cell),
        _ => Ok(false),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;

mod become_cmd;
mod cd;
mod envfilter;
mod envfilter_display;
mod exit;
mod export_fd;
mod exports;
mod read;
mod shift;
mod validation;
mod wait;
