// mutants: -C --lib -- -E 'test(/^intercept::getopts::/)'

//! `getopts <opts> name` — the plumbing (task #589): the name resolves as an
//! in-shell intercept, and the shell state carries the `OPTIND` counter the scan
//! will read. The optstring scanner lands in task #590, the variable settings,
//! diagnostics and exit statuses in task #591.

use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::UintParse;
use sys::fork_cell::ForkCell;

/// The counter's default: `getopts` starts at argument **1**, and bash 5.3.9
/// measures the seed as a visible variable (`set` answers `OPTIND=1`) that is
/// never exported. One owner — [`ShellState`]’s field seed, `unset_var`’s reset,
/// and the string store’s startup row (task #589).
pub(crate) const DEFAULT_OPTIND: u64 = 1;

/// The counter the name `OPTIND` describes, given the value just assigned and the
/// counter before it. Only an **exact** decimal moves it; an unparseable value
/// keeps the counter — bash 5.3.9 measures `OPTIND=abc` as "start at 1" while
/// `$OPTIND` still prints `abc`. The arithmetic-evaluator forms (`0x10`, `010`,
/// `3x`, `-1`) go through bash's arithmetic and are task #590's decision.
pub(crate) fn counter(value: &ShortCStr, current: u64) -> u64 {
    match value.parse_base0_uint() {
        UintParse::Exact(v) => v,
        _ => current,
    }
}

/// Claims `getopts` so the forked launch path stops resolving its command path —
/// today `getopts a o` fails `failed to resolve command path` (POSIX gaps #6.9).
/// The scan itself is task #590: this stub validates (the house rule for
/// in-process commands) and reports success, so **the status is the stub's, not
/// bash's** — bash's `getopts a o` with no operands exits 1. The pin below is
/// marked to move with the scan.
pub(crate) fn run_getopts(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept_no_builtin(line, "getopts", cmdline)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(0);
    Ok(true)
}

#[cfg(test)]
mod tests;
