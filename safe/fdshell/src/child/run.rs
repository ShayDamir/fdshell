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
    args_mask: &[Vec<bool>],
    redirects: &[Redirect],
) -> Result<i32, Report<ChildProcessError>> {
    setup_shellfd(child_sock.as_ref(), cell)?;
    apply_redirects(redirects)?;

    let resolved = substitute_args(args, args_mask, cell)
        .change_context(ChildProcessError::SubstituteFailed)?;
    if let Some(sock) = &child_sock {
        let last = resolved.last().cloned().unwrap_or_else(|| cmd.name.clone());
        crate::last_arg::send(sock, &last).change_context(ChildProcessError::LastArgSend)?;
    }
    let sealed: Vec<sys::ExportedCStr> = resolved.iter().map(|cs| cs.export()).collect();
    let refs: Vec<&CStr> = sealed.iter().map(|rc| rc.as_ref()).collect();

    let state = cell
        .borrow()
        .change_context(ChildProcessError::BorrowFailed)?;

    crate::xtrace::trace(cmd.name.as_bytes().unwrap_or(&[]), &resolved, &state);

    if cmd.builtin || child::dispatch::builtin_first(&cmd.name, &state) {
        run_builtin(&cmd, &refs, args, &state)
    } else {
        external::run_external(&cmd, &refs, &state)
    }
}

fn setup_shellfd(
    sock: Option<&sys::LocalFd>,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<ChildProcessError>> {
    if let Some(s) = sock {
        let mut state = cell
            .borrow_mut()
            .change_context(ChildProcessError::BorrowFailed)?;
        state.shell_sock = Some(
            s.try_clone()
                .change_context(ChildProcessError::ExportFailed)?,
        );
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
