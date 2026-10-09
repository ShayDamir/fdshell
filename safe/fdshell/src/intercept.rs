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
    let result = dispatch::dispatch(cmd, line, cmdline, text, cell)?;
    if let Some(control) = result {
        last_arg_frame::set_intercepted_last_arg(cmdline, cell)?;
        Ok(Some(control))
    } else {
        Ok(None)
    }
}

pub(super) fn handled(ran: bool) -> Option<Option<LoopControl>> {
    if ran { Some(None) } else { None }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;

mod alias_cmd;
mod become_cmd;
mod cd;
mod colon;
pub(crate) mod commands;
mod dispatch;
mod envfilter;
mod envfilter_display;
mod eval_cmd;
mod exit;
mod export_fd;
mod exports;
mod hash_cmd;
mod last_arg_frame;
mod let_cmd;
mod local;
mod read;
mod recvmsg;
mod send_fd;
mod sendmsg;
mod set_cmd;
mod set_limit;
mod set_list;
mod set_short;
mod shift;
mod shopt;
mod signalfd_cmd;
mod source;
mod timeout_cmd;
mod times;
mod ulimit_cmd;
mod validation;
mod wait_cmd;
mod waitpid;
