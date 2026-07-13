use crate::child::Command;
use crate::error::child_process::ChildProcessError;
use crate::exec;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use std::ffi::CStr;

pub(super) fn run_external(
    cmd: &Command,
    refs: &[&CStr],
    state: &ShellState,
) -> Result<i32, Report<ChildProcessError>> {
    let name = sys::RefCStr::from(cmd.name.clone());
    let fd = exec::resolve_path(&name)
        .change_context(ChildProcessError::ResolveFailed(cmd.name.clone()))?;
    let full_argv: Vec<&CStr> = std::iter::once(name.as_ref())
        .chain(refs.iter().copied())
        .collect();
    match exec::exec_fd(
        &fd,
        &full_argv,
        &state.exports,
        &state.env_filter,
        state.shell_sock.as_ref(),
    ) {
        Ok(()) => Ok(0),
        Err(report) => Err(report.change_context(ChildProcessError::ExecFailed)),
    }
}
