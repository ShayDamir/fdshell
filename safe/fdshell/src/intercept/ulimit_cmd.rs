//! `ulimit [-HSa] [-cdeflmnstuv] [limit]` — bash-compatible resource limits.
//!
//! Runs in-shell: limits are per-process and children inherit them, so an
//! intercept is the right home (like bash's own `ulimit` builtin).

use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

mod parse;
mod resources;
mod set;

pub(crate) fn run_ulimit(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "ulimit", cmdline)?;
    let parsed = parse::parse(&cmdline.args)?;
    if parsed.list {
        let out = resources::list(parsed.hard)?;
        sys::OUT.write_all(&out).ok();
    } else {
        let res = parsed.resource.unwrap_or(resources::DEFAULT);
        match parsed.value {
            Some(value) => set::set_limit(res, parsed.hard, parsed.soft, value)?,
            None => {
                let lim = sys::rlimit::get(res.id).change_context(CmdError::UlimitGet)?;
                let raw = if parsed.hard { lim.hard } else { lim.soft };
                let mut out = resources::value_bytes(raw, res).into_bytes();
                out.push(b'\n');
                sys::OUT.write_all(&out).ok();
            }
        }
    }
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(0);
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
