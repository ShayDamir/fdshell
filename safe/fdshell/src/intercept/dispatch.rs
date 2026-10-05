//! The intercepted-command dispatch table.

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::parse::CommandLine;
use crate::state::ShellState;
use error_stack::Report;
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// Route an intercepted command to its handler; `Ok(None)` when `cmd` is not
/// intercepted.
pub(crate) fn dispatch(
    cmd: &[u8],
    line: &[u8],
    cmdline: &CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<Option<LoopControl>>, Report<CmdError>> {
    // Add new commands here AND to `INTERCEPTED_COMMANDS` (commands.rs).
    match cmd {
        b"alias" => super::alias_cmd::run_alias(line, cmdline, text, cell).map(super::handled),
        b"unalias" => super::alias_cmd::run_unalias(line, cmdline, text, cell).map(super::handled),
        b"cd" => super::cd::run_cd(line, cmdline, text, cell).map(super::handled),
        b"exit" | b"quit" => super::exit::run_exit(line, cmdline, cell).map(super::handled),
        b"become" => super::become_cmd::run_become(line, cmdline, cell).map(super::handled),
        b"exec" => super::become_cmd::run_exec(line, cmdline, cell).map(super::handled),
        b"export_fd" => super::export_fd::run_export_fd(line, cmdline, cell).map(super::handled),
        b"waitpid" => super::waitpid::run_waitpid(line, cmdline, cell).map(super::handled),
        b"wait" => super::wait_cmd::run_wait(line, cmdline, cell).map(super::handled),
        b"times" => super::times::run_times(line, cmdline, cell).map(super::handled),
        b"export" => super::exports::run_export(line, cmdline, text, cell).map(super::handled),
        b"eval" => super::eval_cmd::run_eval(line, cmdline, text, cell).map(Some),
        b"source" | b"." => super::source::run_source(line, cmdline, text, cell).map(Some),
        b"envfilter" => super::envfilter::run_envfilter(line, cmdline, cell).map(super::handled),
        b"shift" => super::shift::run_shift(line, cmdline, cell).map(super::handled),
        b"hash" => super::hash_cmd::run_hash(line, cmdline, cell).map(super::handled),
        b"let" => super::let_cmd::run_let(line, cmdline, cell).map(super::handled),
        b"set" => super::set_cmd::run_set(line, cmdline, text, cell).map(super::handled),
        b"shopt" => super::shopt::run_shopt(line, cmdline, text, cell).map(super::handled),
        b"read" => super::read::run_read(line, cmdline, text, cell).map(super::handled),
        b"ulimit" => super::ulimit_cmd::run_ulimit(line, cmdline, cell).map(super::handled),
        b"signalfd" => super::signalfd_cmd::run_signalfd(line, cmdline, cell).map(super::handled),
        b"timeout" => super::timeout_cmd::run_timeout(line, cmdline, cell).map(super::handled),
        b"send_fd" => super::send_fd::run_send_fd(line, cmdline, cell).map(super::handled),
        b"sendmsg" => super::sendmsg::run_sendmsg(line, cmdline, cell).map(super::handled),
        b"recvmsg" => super::recvmsg::run_recvmsg(line, cmdline, text, cell).map(super::handled),
        _ => Ok(None),
    }
}
