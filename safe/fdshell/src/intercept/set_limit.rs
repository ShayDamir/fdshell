//! `set --stdout-capture-limit <bytes>`: configure the `$(…)` stdout capture
//! cap (default [`crate::cmd_subst::MAX_CAPTURED`], 64 MiB).

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

const FLAG: &str = "--stdout-capture-limit";

/// `set --stdout-capture-limit <bytes>`: set the cap to the given decimal
/// byte count. A missing or non-numeric argument is an error; `0` caps any
/// non-empty output.
pub(super) fn run_set_capture_limit(
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let raw = cmdline.args.get(1).cloned().unwrap_or_default();
    let limit = parse_limit(&raw)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_capture_limit(limit);
    state.set_last_exit(0);
    Ok(())
}

fn parse_limit(raw: &ShortCStr) -> Result<usize, Report<CmdError>> {
    let bytes = raw.as_bytes().change_context(CmdError::Resolve)?;
    if bytes.is_empty() {
        bail!(CmdError::CaptureLimitBad {
            command: "set",
            flag: FLAG,
            value: raw.clone(),
        });
    }
    let mut limit = 0usize;
    for &b in bytes {
        if !b.is_ascii_digit() {
            bail!(CmdError::CaptureLimitBad {
                command: "set",
                flag: FLAG,
                value: raw.clone(),
            });
        }
        limit = limit
            .checked_mul(10)
            .and_then(|v| v.checked_add((b - b'0') as usize))
            .ok_or(CmdError::CaptureLimitBad {
                command: "set",
                flag: FLAG,
                value: raw.clone(),
            })?;
    }
    Ok(limit)
}
