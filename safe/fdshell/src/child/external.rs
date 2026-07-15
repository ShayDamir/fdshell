#![allow(clippy::indexing_slicing)]
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
    let name = sys::RefCStr::from(cmd.name.clone());
    let fd = exec::resolve_path(&name)
        .change_context(ChildProcessError::ResolveFailed(cmd.name.clone()))?;
    let name_cstr = core::ffi::CStr::from_bytes_with_nul(name.as_ref().to_bytes_with_nul())
        .change_context(ChildProcessError::Never)?;
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
