// mutants: -C --lib -C --test -C redirect -- -E 'test(/^run::failing::/) | binary(redirect)'

//! The per-command landing of a failed **redirection** (task #179, POSIX XCU
//! 2.5): the report is printed and the current command fails with status 1 —
//! not the errno — and the shell continues with the next statement. Only the
//! conversion sites live elsewhere (the simple-command dispatch, the
//! function-call frame, the block executors, the assignment arms and
//! `run_one`'s brace-guard path); this module owns the classification, the
//! POSIX gate and the conversion itself. The control-flow layers are untouched:
//! the conversion feeds the existing status channel and returns `Ok`, so the
//! gate, errexit and trap machinery run on it as they do on any status.

use core::fmt::Write;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::error::launch::LaunchError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

/// Classification: the report's chain carries a **redirect-layer** frame.
/// Producers `change_context` it, so the frame is the current context at every
/// conversion site (`error_stack::Report` is not `Clone`, so the predicate
/// stays single-frame). Non-redirect
/// frames (usage, expansion, chdir, write, parse) do not classify and stay on
/// the abort channel (the `#179` pins in `tests/hash.rs` and
/// `tests/cd_expand.rs` document those).
pub(crate) fn classifies(report: &Report<CmdError>) -> bool {
    matches!(
        report.current_context(),
        CmdError::Redirect | CmdError::AmbiguousRedirect { .. }
    )
}

/// The same for the launch path, whose report type is `LaunchError`; the match
/// runs before the simple-command path re-contexts it.
pub(crate) fn classifies_launch(report: &Report<LaunchError>) -> bool {
    matches!(report.current_context(), LaunchError::Redirect)
}

/// The POSIX #2.14 special builtins plus `umask` as the UNIX extension; the
/// intercepted names among them are cross-checked against the
/// `intercept/commands.rs` table. A redirect failure on one of these keeps the
/// abort scope (POSIX; bash's default mode continues, bash's `set -o posix`
/// agrees with fdshell).
const SPECIAL: &[&[u8]] = &[
    b":",
    b".",
    b"eval",
    b"exec",
    b"export",
    b"readonly",
    b"set",
    b"shift",
    b"times",
    b"trap",
    b"unset",
    b"break",
    b"continue",
    b"return",
    b"umask",
];

pub(crate) fn is_special(name: &[u8]) -> bool {
    SPECIAL.contains(&name)
}

/// The composition at the function-call frame and the intercepts: a
/// user-defined function (`is_function`) is not a special builtin, and a
/// non-special intercepted command converts; the gate keeps its abort scope.
pub(crate) fn converts(name: &[u8], is_function: bool, report: &Report<CmdError>) -> bool {
    (is_function || !is_special(name)) && classifies(report)
}

/// The conversion: print the report the way the top level and the child path
/// do, and set the command's status to 1, so the caller continues with the
/// next statement.
pub(crate) fn fail<E>(
    report: &Report<E>,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let _ = writeln!(crate::io::Stderr, "{report:?}");
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(1);
    Ok(())
}

/// The boundary one-liner for the block executors, the assignment arms and
/// `run_one`'s brace-guard path: convert when the report classifies, propagate
/// it (the abort scope) when it does not.
pub(crate) fn convert(
    report: Report<CmdError>,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    if !classifies(&report) {
        return Err(report);
    }
    fail(&report, cell)
}

/// The `run_dispatch.rs` assignment-arm wrapper: a bare assignment is not a
/// special builtin, so a classifying report converts (`Ok(None)`) and the
/// caller continues; anything else propagates unchanged.
pub(crate) fn pass<T>(
    result: Result<T, Report<CmdError>>,
    cell: &ForkCell<ShellState>,
) -> Result<Option<T>, Report<CmdError>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(report) if classifies(&report) => {
            fail(&report, cell)?;
            Ok(None)
        }
        Err(report) => Err(report),
    }
}

#[cfg(test)]
mod tests;
