use crate::child::{self, Command, handle_builtin_error};
use crate::error::child_process::ChildProcessError;
use crate::exec;
use crate::redirect::Redirect;
use crate::state::ShellState;
use crate::substitute::substitute_args;
use error_stack::{Report, ResultExt};
use std::ffi::CStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub fn child_main(
    child_sock: Option<sys::LocalFd>,
    cell: &ForkCell<ShellState>,
    cmd: Command,
    args: &[ShortCStr],
    args_fq: &[bool],
    redirects: &[Redirect],
) -> Result<i32, Report<ChildProcessError>> {
    if let Some(sock) = child_sock {
        sock.export_to(sys::shellfd::SHELLFD)
            .change_context(ChildProcessError::RedirectFailed)?;
        sys::shellfd::set_capture_active(true);
    } else {
        sys::shellfd::set_capture_active(false);
    }

    for r in redirects {
        r.export()
            .change_context(ChildProcessError::RedirectFailed)?;
    }

    let resolved =
        substitute_args(args, args_fq, cell).change_context(ChildProcessError::SubstituteFailed)?;
    let refs: Vec<&CStr> = resolved.iter().map(|cs| cs.as_c_str()).collect();

    if cmd.builtin {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::BorrowFailed)?;
        let cmd_name = cmd.name.clone();
        match child::dispatch::dispatch_builtin(cmd.name, &refs, args, &state) {
            Ok(code) => Ok(code),
            Err(report) => handle_builtin_error(cmd_name, report),
        }
    } else {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::BorrowFailed)?;
        let name = sys::RefCStr::from(cmd.name.clone());
        let fd = exec::resolve_path(&name)
            .change_context(ChildProcessError::ResolveFailed(cmd.name.clone()))?;
        let full_argv: Vec<&CStr> = std::iter::once(name.as_ref())
            .chain(refs.iter().copied())
            .collect();
        match exec::exec_fd(&fd, &full_argv, &state.exports, &state.env_filter) {
            Ok(()) => Ok(0),
            Err(report) => Err(report.change_context(ChildProcessError::ExecFailed)),
        }
    }
}
