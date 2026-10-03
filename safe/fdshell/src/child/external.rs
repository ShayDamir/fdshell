use crate::child::Command;
use crate::error::child_process::ChildProcessError;
use crate::exec;
use crate::state::ShellState;
use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ExportedCStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub(super) fn run_external(
    cmd: &Command,
    refs: &[&CStr],
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<ChildProcessError>> {
    // Glob-expand the command name (bash field model): the first match is the
    // command, any further matches are prepended as arguments. A glob matching
    // a builtin/function name runs the file — dispatch saw the raw word.
    let expanded = crate::glob::expand(&cmd.name, &cmd.command_mask, cell)
        .change_context(ChildProcessError::ResolveFailed(cmd.name.clone()))?;
    let (name, extra) = split_command(&expanded, &cmd.name);
    let state = cell
        .borrow()
        .change_context(ChildProcessError::BorrowFailed)?;
    let name_exported = name.export();
    let fd = exec::resolve_path(&name, &state.hash_table)
        .change_context(ChildProcessError::ResolveFailed(cmd.name.clone()))?;
    let name_cstr = name_exported.as_ref();
    let mut full_argv: Vec<&CStr> = alloc::vec![name_cstr];
    for e in &extra {
        full_argv.push(e.as_ref());
    }
    for r in refs {
        full_argv.push(*r);
    }
    match exec::exec_fd(
        &fd,
        &full_argv,
        &state.environ,
        &state.exports,
        &state.env_filter,
        state.shell_sock.as_ref(),
    ) {
        Ok(()) => Ok(0),
        Err(report) => Err(report.change_context(ChildProcessError::ExecFailed)),
    }
}

/// Split a command-name expansion into (command, leading args) per the bash
/// field model: the first field is the command, the rest are arguments. An
/// empty expansion (nullglob, no match) falls back to the literal word.
fn split_command(expanded: &[ShortCStr], literal: &ShortCStr) -> (ShortCStr, Vec<ExportedCStr>) {
    let Some(first) = expanded.first() else {
        return (literal.clone(), Vec::new());
    };
    let extra = expanded.iter().skip(1).map(|s| s.export()).collect();
    (first.clone(), extra)
}

#[cfg(test)]
mod tests;
