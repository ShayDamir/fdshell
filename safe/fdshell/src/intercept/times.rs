use alloc::format;
use alloc::string::String;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

/// `times` — the shell's and the reaped children's accumulated user/sys CPU
/// times, in bash's layout (tabs separate the columns).
pub(crate) fn run_times(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept_no_builtin(line, "times", cmdline)?;
    let self_t = sys::getrusage::self_usage().change_context(CmdError::TimesUsage)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let out = format!(
        "\tUser time\tSystem time\n\t{}\t{}\n\tChildren user time\tChildren system time\n\t{}\t{}\n",
        centis(self_t.utime),
        centis(self_t.stime),
        centis(state.child_utime),
        centis(state.child_stime),
    );
    sys::OUT.write_all(out.as_bytes()).ok();
    state.set_last_exit(0);
    Ok(true)
}

/// Microseconds as bash's `N.NN` centiseconds.
fn centis(us: u64) -> String {
    format!("{}.{:02}", us / 10_000, (us / 100) % 100)
}

#[cfg(test)]
mod tests;
