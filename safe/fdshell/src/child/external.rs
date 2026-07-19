use crate::child::Command;
use crate::error::child_process::ChildProcessError;
use crate::exec;
use crate::state::ShellState;
use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};

pub(super) fn run_external(
    cmd: &Command,
    refs: &[&CStr],
    state: &ShellState,
) -> Result<i32, Report<ChildProcessError>> {
    let name_exported = cmd.name.export();
    let fd = exec::resolve_path(&cmd.name)
        .change_context(ChildProcessError::ResolveFailed(cmd.name.clone()))?;
    let name_cstr = name_exported.as_ref();
    let mut full_argv: Vec<&CStr> = alloc::vec![name_cstr];
    for r in refs {
        full_argv.push(*r);
    }
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
