// mutants: -C --lib -C --test -C trap -- -E 'test(/^intercept::trap_cmd::/) | binary(trap)'

//! `trap [action [condition ...]]` — the shell's action table: register a command
//! string for `EXIT`, `0` (POSIX's spelling of the same condition) or a signal,
//! `""` to ignore one, `-` to give it back to the kernel, and print the table when
//! no action is given. It runs **in-process**: a disposition installed by a forked
//! child would die with the child, so the registration and the `sigaction` both
//! happen here (the table is task #583, the wrapper is task #582).
//!
//! Storing is this task's whole job: running a stored action on delivery or at
//! exit is the deferred executor's (task #585). The list form is already the
//! inverse of the parser — the `'`-quoting, the `--` marker and the name spelling
//! are one contract, and running `eval "$(trap)"` re-inputs what was printed.
//!
//! POSIX XCU 2.15, cross-checked against bash 5.3.9 (measured 2026-11-16).

mod cond;
mod list;
mod parse;

use alloc::vec::Vec;
use core::fmt::Write;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ScriptText, ShortCStr};

use crate::error::cmd::CmdError;
use crate::state::{EXIT, ShellState, TrapDisposition};

/// bash 5.3.9's measured numbers: an invalid name or number answers 1, a usage
/// form answers 2, and in both cases the script carries on.
const BAD_CONDITION_STATUS: i32 = 1;
const USAGE_STATUS: i32 = 2;

/// The dispatch arm's handler: validate, expand the words, run one form. The
/// expanded field list goes back to the frame, so `$_` is the last expanded word.
pub(crate) fn run_trap(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    _text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<Vec<ShortCStr>>, Report<CmdError>> {
    super::validation::validate_intercept(line, "trap", cmdline)?;
    // The expansion re-borrows the `ForkCell` (IFS, `dotglob`, `%fd`), so it runs
    // **before** the state borrow (LESSONS "Never hold a ForkCell borrow across a
    // recursive call that re-borrows").
    let words = crate::substitute::substitute_args(
        &cmdline.args,
        &cmdline.args_mask,
        &cmdline.args_span,
        &cmdline.args_quoted,
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let status = match run(&words, &mut state) {
        Ok(status) => status,
        Err(report) => report_and_continue(report),
    };
    state.set_last_exit(status);
    Ok(Some(words))
}

/// The two forms, split by the grammar: no action operand at all is the list form.
fn run(words: &[ShortCStr], state: &mut ShellState) -> Result<i32, Report<CmdError>> {
    let Some(request) = parse::form(words)? else {
        list::print(state)?;
        return Ok(0);
    };
    apply(&request, state)
}

/// Register one call, condition by condition: POSIX reports a bad operand and
/// keeps the conditions before it (measured: `trap 'x' USR1 BADSIG` registers
/// `USR1`, answers 1, and `trap` still lists it).
fn apply(request: &parse::Request, state: &mut ShellState) -> Result<i32, Report<CmdError>> {
    for field in request.conditions.iter() {
        let number = cond::condition(field)?;
        if refused_by_entry(number, state) {
            continue;
        }
        state.set_trap(number, request.disposition.clone());
        if number != EXIT {
            sys::signal::set_action(number, disposition_action(&request.disposition))
                .change_context(CmdError::TrapSigaction)?;
        }
    }
    Ok(0)
}

/// The disposition `sigaction` gets for one table entry: the table alone would be
/// a promise, so every registration (and every reset) installs it as well.
fn disposition_action(disposition: &TrapDisposition) -> sys::signal::Action {
    match disposition {
        TrapDisposition::Ignore => sys::signal::Action::Ignore,
        TrapDisposition::Action(_) => sys::signal::Action::Record,
        TrapDisposition::Default => sys::signal::Action::Default,
    }
}

/// POSIX XCU 2.15's entry rule: in a non-interactive shell, a signal ignored on
/// entry cannot be trapped or reset, and no error need be reported. fdshell's
/// inherited set is not empty the way bash's is — Rust's runtime ignores `SIGPIPE`
/// — so this refusal is reachable, and `EXIT` is not a signal.
fn refused_by_entry(cond: i32, state: &ShellState) -> bool {
    !state.interactive && cond != EXIT && state.ignored_on_entry & (1u64 << (cond - 1)) != 0
}

/// Report and continue: an invalid operand and a usage form must not abort the
/// shell (POSIX), so the report is printed here and the status is the answer —
/// the usage forms have bash's own number, which the runner would not give them.
fn report_and_continue(report: Report<CmdError>) -> i32 {
    let _ = writeln!(crate::io::Stderr, "{report:?}");
    if matches!(report.current_context(), CmdError::TrapUsage) {
        USAGE_STATUS
    } else {
        BAD_CONDITION_STATUS
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
