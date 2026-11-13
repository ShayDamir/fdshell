// mutants: -C --lib -C --test -C umask -- -E 'test(/^run_dispatch::umask::/) | binary(umask)'

//! `umask`'s run arm (task #554): the operand is applied here, where the statement
//! actually runs, and the report is what the flags ask for.
//!
//! The octal operand is the only one that reaches the syscall unchanged; a symbolic
//! operand was parsed into a plan and is evaluated against the mask this process
//! already has (POSIX: the operand is a chmod-style mode, and chmod writes the bits
//! it names onto the complement). Without an operand the current mask is printed.
//! With one, POSIX XCU 2.95 wants no output — bash prints whenever `-S` asked for
//! the symbolic form, and fdshell follows bash (the divergence is recorded in
//! `POSIX_COMPATIBILITY.md`). Either way the run succeeded, so `$?` is 0.

use crate::error::cmd::CmdError;
use crate::parse::umask_mode::UmaskMode;
use crate::state::ShellState;
use alloc::format;
use alloc::string::String;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

use crate::parse::umask_mode::symbolic::symbolic_text;

/// Apply the operand (if any), report what the flags ask for, leave `$?` = 0.
pub(super) fn run(
    mode: Option<&UmaskMode>,
    dash_p: bool,
    dash_s: bool,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    if let Some(mode) = mode {
        apply(mode);
    }
    if let Some(text) = report(mode.is_some(), dash_p, dash_s, sys::umask::get()) {
        sys::OUT.write_all(text.as_bytes()).ok();
    }
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(0);
    Ok(())
}

/// The operand write: octal as given (bash's octal-range check is not fdshell's,
/// task #472), symbolic evaluated on the complement of the mask it reads.
fn apply(mode: &UmaskMode) {
    match mode {
        UmaskMode::Octal(value) => {
            sys::umask::set(*value);
        }
        UmaskMode::Symbolic(symbolic) => {
            let next = symbolic.apply(sys::umask::get());
            sys::umask::set(next);
        }
    }
}

/// The line to print, newline included, or `None` when nothing is printed.
/// Without an operand the mask is reported in octal, or symbolically under `-S`,
/// and `-p` puts the reusable `umask …` prefix in front of either (bash 5.3.9,
/// measured: `-p -S` prints `umask -S u=rwx,g=rx,o=rx`).
fn report(applied: bool, dash_p: bool, dash_s: bool, mask: u32) -> Option<String> {
    if applied {
        return dash_s.then(|| with_newline(&symbolic_text(mask)));
    }
    let body = if dash_s {
        symbolic_text(mask)
    } else {
        format!("{mask:04o}")
    };
    let line = if dash_p {
        let prefix = if dash_s { "umask -S " } else { "umask " };
        format!("{prefix}{body}")
    } else {
        body
    };
    Some(with_newline(&line))
}

/// The report's trailing newline is script-visible (task #472); the print failure is
/// not the run's business, so the write is `.ok()`-tolerant as everywhere else.
fn with_newline(text: &str) -> String {
    let mut text = String::from(text);
    text.push('\n');
    text
}

#[cfg(test)]
mod tests;
