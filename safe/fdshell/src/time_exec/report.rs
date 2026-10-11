//! The `time` report: the start snapshot, the two layouts, and the write to
//! stderr.

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use alloc::format;
use alloc::string::String;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

/// A statement's start snapshot, in microseconds.
pub struct Start {
    pub real: u64,
    pub user: u64,
    pub sys: u64,
}

/// Snapshot the clock and the CPU times a `time` report measures against:
/// the shell's own rusage plus the CPU of the children it reaped (Linux's
/// `getrusage(RUSAGE_SELF)` excludes reaped children, so their `waitid`
/// rusage totals are added — the closest Linux analogue of POSIX's
/// `tms_utime + tms_cutime`). The `ShellState` borrow ends with this function.
pub fn start(cell: &ForkCell<ShellState>) -> Result<Start, Report<CmdError>> {
    let real = sys::clock::monotonic_us().change_context(CmdError::TimeClock)?;
    let own = sys::getrusage::self_usage().change_context(CmdError::TimeUsage)?;
    let state = cell.borrow().change_context(CmdError::Never)?;
    Ok(Start {
        real,
        user: own.utime + state.child_utime,
        sys: own.stime + state.child_stime,
    })
}

/// Print the report to stderr and hand back the inner statement's result:
/// the status and control flow belong to the inner, the report is attached.
/// Infallible by design (§4.12): a failed stderr write is not user-fixable.
pub fn emit(
    posix: bool,
    start: &Start,
    cell: &ForkCell<ShellState>,
    result: Result<Option<LoopControl>, Report<CmdError>>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let now = real_now();
    let own = sys::getrusage::self_usage().unwrap_or_default();
    let (child_user, child_sys) = match cell.borrow() {
        Ok(state) => (state.child_utime, state.child_stime),
        Err(_) => (0, 0),
    };
    let out = report(
        posix,
        now.saturating_sub(start.real),
        (own.utime + child_user).saturating_sub(start.user),
        (own.stime + child_sys).saturating_sub(start.sys),
    );
    sys::ERR.write_all(out.as_bytes()).ok();
    result
}

/// The report text: the POSIX `-p` layout, or bash's default (a leading empty
/// line, tab-separated `N m N.NNN s` values).
fn report(posix: bool, real: u64, user: u64, sys: u64) -> String {
    if posix {
        return format!(
            "real {}\nuser {}\nsys {}\n",
            seconds(real),
            seconds(user),
            seconds(sys)
        );
    }
    format!(
        "\nreal\t{}\nuser\t{}\nsys\t{}\n",
        clock(real),
        clock(user),
        clock(sys)
    )
}

/// Microseconds as bash's `N.NN` centiseconds (the unit the `times` builtin
/// prints, which pins the same conversion).
pub(crate) fn centis(us: u64) -> String {
    format!("{}.{:02}", us / 10_000, (us / 100) % 100)
}

/// Microseconds as the POSIX `time -p` unit: whole seconds plus two fractional
/// digits (`real 0.10` for a tenth of a second).
pub(crate) fn seconds(us: u64) -> String {
    format!("{}.{:02}", us / 1_000_000, (us / 10_000) % 100)
}

/// Microseconds as bash's default-layout `N m N.NNN s` (integer math).
pub(crate) fn clock(us: u64) -> String {
    format!(
        "{}m{}.{:03}s",
        us / 60_000_000,
        (us / 1_000_000) % 60,
        (us / 1_000) % 1_000
    )
}

/// The monotonic clock, with a zero fallback: a clock read that fails leaves
/// `real` at 0, which is all a failed report write can offer anyway.
fn real_now() -> u64 {
    sys::clock::monotonic_us().unwrap_or(0)
}

#[cfg(test)]
mod tests;
