// mutants: -C --lib -C --test -C alias -C --test -C cd_expand -C --test -C colon -C --test -C group -C --test -C hash -C --test -C help -C --test -C heredoc -C --test -C last_arg -C --test -C loops -C --test -C pipeline -C --test -C positional -C --test -C read -C --test -C recvmsg -C --test -C redirect_scope -C --test -C sendmsg -C --test -C set_cmd -C --test -C set_opts -C --test -C shopt -C --test -C signalfd -C --test -C source -C --test -C strict -C --test -C test -C --test -C timeout -C --test -C times -C --test -C trap -C --test -C ulimit -C --test -C unset -C --test -C wait -C --test -C wordsplit -C --test -C xtrace -- -E 'test(/^intercept::tests::/) | binary(alias) | binary(cd_expand) | binary(colon) | binary(group) | binary(hash) | binary(help) | binary(heredoc) | binary(last_arg) | binary(loops) | binary(pipeline) | binary(positional) | binary(read) | binary(recvmsg) | binary(redirect_scope) | binary(sendmsg) | binary(set_cmd) | binary(set_opts) | binary(shopt) | binary(signalfd) | binary(source) | binary(strict) | binary(test) | binary(timeout) | binary(times) | binary(trap) | binary(ulimit) | binary(unset) | binary(wait) | binary(wordsplit) | binary(xtrace)'

//! `signalfd %var <sig1> [sig2 ...] [--flags F]` — trap signals as an fd source.
//!
//! Runs in-shell: the signalfd must be created in the shell process (the
//! signals are blocked here and delivered via the fd), so an intercept is the
//! right home — a forked child would block the signals in itself and exit.

use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

mod parse;
pub(crate) mod signals;

pub(crate) fn run_signalfd(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "signalfd", cmdline)?;
    let parsed = parse::parse(&cmdline.args)?;
    let name = parsed
        .var
        .strip_prefix(b"%")
        .ok_or(CmdError::SignalfdNoVar)?;
    let fd = sys::signalfd::signalfd(&parsed.signals, parsed.flags)
        .change_context(CmdError::SignalfdSyscall)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_fd_var(
        name,
        crate::state::FdVar {
            fd,
            trace: sys::Trace::boundary(sys::Origin::Shell),
        },
    );
    state.set_last_exit(0);
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
