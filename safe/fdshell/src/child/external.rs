use crate::child::Command;
use crate::error::child_process::ChildProcessError;
use crate::exec;
use crate::state::ShellState;
use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

/// Run the external program named by `cmd.name`.
///
/// Word 0 reached here fully expanded: `command_word` ran the substitution,
/// IFS-splitting and pathname-expansion passes on it in the parent, so the
/// first field of the bash field model *is* the command name and its leading
/// arguments are already in the child's `args`. Nothing is left to expand —
/// globbing the name again would glob an expansion output byte a second time.
pub(super) fn run_external(
    cmd: &Command,
    refs: &[&CStr],
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<ChildProcessError>> {
    let state = cell
        .borrow()
        .change_context(ChildProcessError::BorrowFailed)?;
    let name_exported = cmd.name.export();
    let name_cstr = name_exported.as_ref();
    let fd = exec::resolve_path(&cmd.name, &state.hash_table, state.strict(), state.cwd_fd())
        .change_context(ChildProcessError::ResolveFailed(cmd.name.clone()))?;
    let mut full_argv: Vec<&CStr> = alloc::vec![name_cstr];
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
