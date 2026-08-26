use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// `None`: not intercepted; `Some(control)`: handled, with the control to propagate.
pub(crate) fn try_intercept(
    text: &ScriptText,
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<Option<Option<LoopControl>>, Report<CmdError>> {
    let line = text.as_bytes().change_context(CmdError::Never)?;
    let cmd = cmdline.command.as_bytes().change_context(CmdError::Never)?;
    // `set` traces itself once it has decided which form ran, so a
    // fall-through `set` is not traced.
    if !cmd.eq(b"set") {
        crate::xtrace::trace_cmd(cmd, cmdline, cell);
    }
    let result = match cmd {
        b"alias" => alias_cmd::run_alias(line, cmdline, text, cell).map(handled),
        b"unalias" => alias_cmd::run_unalias(line, cmdline, text, cell).map(handled),
        b"cd" => cd::run_cd(line, cmdline, text, cell).map(handled),
        b"exit" | b"quit" => exit::run_exit(line, cmdline, cell).map(handled),
        b"become" => become_cmd::run_become(line, cmdline, cell).map(handled),
        b"exec" => become_cmd::run_exec(line, cmdline, cell).map(handled),
        b"export_fd" => export_fd::run_export_fd(line, cmdline, cell).map(handled),
        b"waitpid" => waitpid::run_waitpid(line, cmdline, cell).map(handled),
        b"export" => exports::run_export(line, cmdline, text, cell).map(handled),
        b"eval" => eval_cmd::run_eval(line, cmdline, text, cell).map(Some),
        b"source" | b"." => source::run_source(line, cmdline, text, cell).map(Some),
        b"envfilter" => envfilter::run_envfilter(line, cmdline, cell).map(handled),
        b"shift" => shift::run_shift(line, cmdline, cell).map(handled),
        b"set" => set_cmd::run_set(line, cmdline, text, cell).map(handled),
        b"shopt" => shopt::run_shopt(line, cmdline, text, cell).map(handled),
        b"read" => read::run_read(line, cmdline, text, cell).map(handled),
        b"send_fd" => send_fd::run_send_fd(line, cmdline, cell).map(handled),
        _ => return Ok(None),
    };
    if let Some(control) = result? {
        last_arg_frame::set_intercepted_last_arg(cmdline, cell)?;
        Ok(Some(control))
    } else {
        Ok(None)
    }
}

fn handled(ran: bool) -> Option<Option<LoopControl>> {
    if ran { Some(None) } else { None }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;

mod alias_cmd;
mod become_cmd;
mod cd;
mod envfilter;
mod envfilter_display;
mod eval_cmd;
mod exit;
mod export_fd;
mod exports;
mod last_arg_frame;
mod read;
mod send_fd;
mod set_cmd;
mod set_list;
mod shift;
mod shopt;
mod source;
mod validation;
mod waitpid;
