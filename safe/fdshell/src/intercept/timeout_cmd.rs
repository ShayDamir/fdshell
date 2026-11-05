//! `timeout <seconds> <cmd> [args ...]` — run a command with a wall-clock deadline.
//!
//! Runs in-shell: the command is launched as a child and the shell polls the
//! child's pidfd against a one-shot deadline timer. On timeout the child is
//! sent SIGTERM, then SIGKILL if it survives a grace period, and the command
//! exits 124 (matching coreutils `timeout`). The argument parse lives in
//! `timeout_cmd/parse.rs`, the deadline poll and the terminate sequence in
//! `timeout_cmd/wait.rs`.

use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::error::cmd::timeout::TimeoutError;
use crate::parse::BuiltinPrefix;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

mod parse;
mod wait;

pub(crate) fn run_timeout(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<TimeoutError>> {
    // The shared validator answers with `CmdError`; `timeout`'s type carries the
    // same two messages, so the conversion is per-variant (STYLE §4.5).
    super::validation::validate_intercept(line, "timeout", cmdline).map_err(
        |report| match *report.current_context() {
            CmdError::CapturesNotSupported { command } => {
                report.change_context(TimeoutError::CapturesNotSupported { command })
            }
            _ => report
                .change_context(TimeoutError::BuiltinKeywordNotSupported { command: "timeout" }),
        },
    )?;
    let cfg = parse::parse(
        &cmdline.args,
        &cmdline.args_mask,
        &cmdline.args_span,
        &cmdline.args_quoted,
    )?;
    let subcmdline = crate::parse::CommandLine {
        prefix: BuiltinPrefix::None,
        command: cfg.command,
        command_mask: cfg.command_mask,
        command_span: cfg.command_span,
        command_quoted: cfg.command_quoted,
        // A scoped prefix on `timeout` itself rides in the forked child's
        // inherited state, not in this sub-command line.
        env_assigns: vec![],
        args: cfg.args,
        args_mask: cfg.args_mask,
        args_span: cfg.args_span,
        args_quoted: cfg.args_quoted,
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    };
    // The target word is word 0, so it is expanded exactly like a command word
    // — substitution, IFS splitting, globbing — before the launch (task #268).
    // The shared expansion answers with `CmdError`; the conversion is per-variant
    // so both messages survive verbatim (STYLE §4.5, §4.17).
    let subcmdline = crate::command_word::expand(&subcmdline, cell)
        .map_err(|report| match *report.current_context() {
            CmdError::Never => report.change_context(TimeoutError::Never),
            _ => report.change_context(TimeoutError::Resolve),
        })?
        .into_owned();
    let outcome = crate::launch::launch(cell, &subcmdline, &Vec::new())
        .change_context(TimeoutError::TimeoutLaunch)?;
    let (exit, times) = wait::bounded_wait(&outcome.pidfd, cfg.seconds)?;
    let mut state = cell.borrow_mut().change_context(TimeoutError::Never)?;
    state.add_child_times(times);
    state.set_last_exit(exit);
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
