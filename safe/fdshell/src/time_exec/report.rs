//! The `time` report: the start snapshot, the two layouts, and the write to
//! stderr.

mod format;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use format::report;
use sys::fork_cell::ForkCell;

pub(crate) use format::centis;

/// A snapshot of the clock and the CPU times a `time` report measures, in
/// microseconds: monotonic real time, plus user/sys as the shell's own rusage
/// and the CPU of the children it has reaped (Linux's `getrusage(RUSAGE_SELF)`
/// excludes reaped children, so their `waitid` rusage totals are added — the
/// closest Linux analogue of POSIX's `tms_utime + tms_cutime`).
pub struct Start {
    pub real: u64,
    pub user: u64,
    pub sys: u64,
}

impl Start {
    /// The elapsed part between this snapshot and a later one: the report's
    /// three numbers. Saturating, so a clock that went backwards reports 0.
    pub(super) fn elapsed(&self, end: &Self) -> Self {
        Self {
            real: end.real.saturating_sub(self.real),
            user: end.user.saturating_sub(self.user),
            sys: end.sys.saturating_sub(self.sys),
        }
    }
}

/// Snapshot the clock and the CPU times a `time` report measures against. The
/// `ShellState` borrow ends with this function (never hold a `ForkCell` borrow
/// across a call that re-borrows it).
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

/// The same snapshot taken infallibly — the report's end point, where a failed
/// read can only zero the report (§4.12: a user cannot fix a failed report).
pub(super) fn end(cell: &ForkCell<ShellState>) -> Start {
    let own = sys::getrusage::self_usage().unwrap_or_default();
    let (child_user, child_sys) = match cell.borrow() {
        Ok(state) => (state.child_utime, state.child_stime),
        Err(_) => (0, 0),
    };
    Start {
        real: sys::clock::monotonic_us().unwrap_or(0),
        user: own.utime + child_user,
        sys: own.stime + child_sys,
    }
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
    let elapsed = start.elapsed(&end(cell));
    let out = report(posix, elapsed.real, elapsed.user, elapsed.sys);
    sys::ERR.write_all(out.as_bytes()).ok();
    result
}

#[cfg(test)]
mod tests;
