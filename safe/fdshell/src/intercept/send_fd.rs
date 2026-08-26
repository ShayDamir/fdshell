use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::state::ShellState;

/// `send_fd [tag] %var` — send the fd var to the enclosing capture socket with
/// the given tag (the var name when untagged). Runs in-process, so a `wait`
/// arm uses it to return fds to the parent's bounded-capture array.
pub(crate) fn run_send_fd(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "send_fd", cmdline)?;
    let (tag, vname) = parse_args(&cmdline.args)?;
    {
        let state = cell.borrow().change_context(CmdError::Never)?;
        let sock = state.shell_sock.as_ref().ok_or(CmdError::FdPass)?;
        let fdvar = state.fds.get(&vname).ok_or(CmdError::FdNotSet)?;
        sys::shellfd::send_fd(sock, &fdvar.fd, tag.export()).change_context(CmdError::FdPass)?;
    }
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(0);
    Ok(true)
}

/// `send_fd %var` or `send_fd <tag> %var`.
fn parse_args(args: &[ShortCStr]) -> Result<(ShortCStr, ShortCStr), Report<CmdError>> {
    match args {
        [a] => {
            let v = a.strip_prefix(b"%").ok_or(CmdError::FdPass)?;
            Ok((v.clone(), v.clone()))
        }
        [t, v] => {
            let v = v.strip_prefix(b"%").ok_or(CmdError::FdPass)?;
            Ok((t.clone(), v.clone()))
        }
        _ => bail!(CmdError::FdPass),
    }
}

#[cfg(test)]
mod tests;
