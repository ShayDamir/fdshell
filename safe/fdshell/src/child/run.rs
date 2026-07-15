use crate::child::{self, Command, external, handle_builtin_error};
use crate::error::child_process::ChildProcessError;
use crate::redirect::Redirect;
use crate::state::ShellState;
use crate::substitute::substitute_args;
use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
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
    setup_shellfd(child_sock, cell)?;
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
        external::run_external(&cmd, &refs, &state)
    }
}

fn setup_shellfd(
    sock: Option<sys::LocalFd>,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<ChildProcessError>> {
    if let Some(s) = sock {
        let mut state = cell
            .borrow_mut()
            .change_context(ChildProcessError::BorrowFailed)?;
        state.shell_sock = Some(s);
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
