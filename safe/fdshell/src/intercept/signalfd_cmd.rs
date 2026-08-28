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
mod signals;

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
