use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ScriptText, ShortCStr};

/// The control a handler propagates, and the **expanded** field list it consumed
/// (`None` when no substitution pipeline ran).
type Handled = (Option<LoopControl>, Option<Vec<ShortCStr>>);

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
    if let Some((control, expanded)) = result {
        last_arg_frame::set_intercepted_last_arg(cmdline, expanded.as_deref(), cell)?;
        Ok(Some(control))
    } else {
        Ok(None)
    }
}

/// The table's adapter for handlers that consume raw words: no field list, so
/// `$_` follows the raw rule.
pub(super) fn handled(ran: bool) -> Option<Handled> {
    if ran { Some((None, None)) } else { None }
}

/// The table's adapter for handlers whose words went through the pipeline: they
/// hand their field list back, so `$_` is the last **expanded** field.
pub(super) fn handled_fields(fields: Option<Vec<ShortCStr>>) -> Option<Handled> {
    Some((None, fields))
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
mod set_positional;
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
