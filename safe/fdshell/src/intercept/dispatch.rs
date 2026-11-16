// mutants: -C --lib -C --test -C alias -C --test -C cd_expand -C --test -C colon -C --test -C group -C --test -C hash -C --test -C help -C --test -C heredoc -C --test -C last_arg -C --test -C loops -C --test -C pipeline -C --test -C positional -C --test -C read -C --test -C recvmsg -C --test -C redirect_scope -C --test -C sendmsg -C --test -C set_cmd -C --test -C set_opts -C --test -C shopt -C --test -C signalfd -C --test -C source -C --test -C strict -C --test -C test -C --test -C timeout -C --test -C times -C --test -C trap -C --test -C ulimit -C --test -C unset -C --test -C wait -C --test -C wordsplit -C --test -C xtrace -- -E 'test(/^intercept::tests::/) | binary(alias) | binary(cd_expand) | binary(colon) | binary(group) | binary(hash) | binary(help) | binary(heredoc) | binary(last_arg) | binary(loops) | binary(pipeline) | binary(positional) | binary(read) | binary(recvmsg) | binary(redirect_scope) | binary(sendmsg) | binary(set_cmd) | binary(set_opts) | binary(shopt) | binary(signalfd) | binary(source) | binary(strict) | binary(test) | binary(timeout) | binary(times) | binary(trap) | binary(ulimit) | binary(unset) | binary(wait) | binary(wordsplit) | binary(xtrace)'

//! The intercepted-command dispatch table.

use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// Route an intercepted command to its handler; `Ok(None)` when `cmd` is not
/// intercepted. The handler's own control and field list travel together (see
/// `super::Handled`).
pub(crate) fn dispatch(
    cmd: &[u8],
    line: &[u8],
    cmdline: &CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<super::Handled>, Report<CmdError>> {
    // Add new commands here AND to `INTERCEPTED_COMMANDS` (commands.rs).
    match cmd {
        b"alias" => super::alias_cmd::run_alias(line, cmdline, text, cell).map(super::handled),
        b"unalias" => super::alias_cmd::run_unalias(line, cmdline, text, cell).map(super::handled),
        b"cd" => super::cd::run_cd(line, cmdline, text, cell).map(super::handled_fields),
        b":" => super::colon::run_colon(line, cmdline, cell).map(super::handled),
        b"exit" | b"quit" => super::exit::run_exit(line, cmdline, cell).map(super::handled),
        b"become" => super::become_cmd::run_become(line, cmdline, cell).map(super::handled),
        b"exec" => super::become_cmd::run_exec(line, cmdline, cell).map(super::handled),
        b"export_fd" => super::export_fd::run_export_fd(line, cmdline, cell).map(super::handled),
        b"waitpid" => super::waitpid::run_waitpid(line, cmdline, cell).map(super::handled),
        b"wait" => super::wait_cmd::run_wait(line, cmdline, cell).map(super::handled_fields),
        b"times" => super::times::run_times(line, cmdline, cell).map(super::handled),
        b"export" => super::exports::run_export(line, cmdline, text, cell).map(super::handled),
        b"eval" => super::eval_cmd::run_eval(line, cmdline, text, cell).map(Some),
        b"source" | b"." => super::source::run_source(line, cmdline, text, cell).map(Some),
        b"envfilter" => super::envfilter::run_envfilter(line, cmdline, cell).map(super::handled),
        b"shift" => super::shift::run_shift(line, cmdline, cell).map(super::handled),
        b"hash" => super::hash_cmd::run_hash(line, cmdline, cell).map(super::handled_fields),
        b"let" => super::let_cmd::run_let(line, cmdline, cell).map(super::handled_fields),
        b"local" => super::local::run_local(line, cmdline, text, cell).map(super::handled_fields),
        b"set" => super::set_cmd::run_set(line, cmdline, text, cell).map(super::handled),
        b"shopt" => super::shopt::run_shopt(line, cmdline, text, cell).map(super::handled),
        b"read" => super::read::run_read(line, cmdline, text, cell).map(super::handled),
        b"ulimit" => super::ulimit_cmd::run_ulimit(line, cmdline, cell).map(super::handled),
        b"signalfd" => super::signalfd_cmd::run_signalfd(line, cmdline, cell).map(super::handled),
        b"timeout" => super::timeout_cmd::run_timeout(line, cmdline, cell)
            .map(super::handled)
            .change_context(CmdError::Parse),
        b"trap" => super::trap_cmd::run_trap(line, cmdline, text, cell).map(super::handled_fields),
        b"send_fd" => super::send_fd::run_send_fd(line, cmdline, cell).map(super::handled),
        b"sendmsg" => super::sendmsg::run_sendmsg(line, cmdline, cell).map(super::handled),
        b"recvmsg" => super::recvmsg::run_recvmsg(line, cmdline, text, cell).map(super::handled),
        _ => Ok(None),
    }
}
