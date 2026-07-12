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
    setup_shellfd(child_sock)?;
    apply_redirects(redirects)?;

    let resolved =
        substitute_args(args, args_fq, cell).change_context(ChildProcessError::SubstituteFailed)?;
    let refs: Vec<&CStr> = resolved.iter().map(|cs| cs.as_c_str()).collect();

    let state = cell
        .borrow()
        .change_context(ChildProcessError::BorrowFailed)?;

    if cmd.builtin {
        run_builtin(&cmd, &refs, args, &state)
    } else {
        run_external(&cmd, &refs, &state)
    }
}

fn setup_shellfd(sock: Option<sys::LocalFd>) -> Result<(), Report<ChildProcessError>> {
    if let Some(s) = sock {
        s.export_to(sys::shellfd::SHELLFD)
            .change_context(ChildProcessError::RedirectFailed)?;
        sys::shellfd::set_capture_active(true);
    } else {
        sys::shellfd::set_capture_active(false);
    }
    Ok(())
}

fn apply_redirects(redirects: &[Redirect]) -> Result<(), Report<ChildProcessError>> {
    for r in redirects {
        r.export()
            .change_context(ChildProcessError::RedirectFailed)?;
    }
    Ok(())
}

fn run_builtin(
    cmd: &Command,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<ChildProcessError>> {
    match child::dispatch::dispatch_builtin(cmd.name.clone(), refs, args, state) {
        Ok(code) => Ok(code),
        Err(report) => handle_builtin_error(cmd.name.clone(), report),
    }
}

fn run_external(
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
    match exec::exec_fd(&fd, &full_argv, &state.exports, &state.env_filter) {
        Ok(()) => Ok(0),
        Err(report) => Err(report.change_context(ChildProcessError::ExecFailed)),
    }
}
