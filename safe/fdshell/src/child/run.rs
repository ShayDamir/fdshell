use crate::child::{self, Command, external};
use crate::error::child_process::ChildProcessError;
use crate::parse::CommandLine;
use crate::redirect::Redirect;
use crate::state::ShellState;
use crate::substitute::substitute_args;
use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

pub fn child_main(
    child_sock: Option<sys::LocalFd>,
    cell: &ForkCell<ShellState>,
    cmdline: &CommandLine,
    redirects: &[Redirect],
    env: crate::run_env::EnvAssigns,
) -> Result<i32, Report<ChildProcessError>> {
    let cmd = Command::from(cmdline);
    let args = &cmdline.args;
    setup_shellfd(child_sock.as_ref(), cell)?;
    apply_redirects(redirects)?;

    let resolved = substitute_args(args, &cmdline.args_mask, &cmdline.args_quoted, cell)
        .change_context(ChildProcessError::SubstituteFailed)?;
    if let Some(sock) = &child_sock {
        let last = resolved.last().cloned().unwrap_or_else(|| cmd.name.clone());
        crate::last_arg::send(sock, &last).change_context(ChildProcessError::LastArgSend)?;
    }
    // The scoped prefix applies after substitution: the command's own words
    // were split with the parent's IFS; only its execution environment sees
    // a scoped `IFS` (`IFS=: echo a:b` → `a:b`).
    crate::run_env::apply_child(&env, cell).change_context(ChildProcessError::BorrowFailed)?;
    let sealed: Vec<sys::ExportedCStr> = resolved.iter().map(|cs| cs.export()).collect();
    let refs: Vec<&CStr> = sealed.iter().map(|rc| rc.as_ref()).collect();

    let state = cell
        .borrow()
        .change_context(ChildProcessError::BorrowFailed)?;

    crate::xtrace::trace(cmd.name.as_bytes().unwrap_or(&[]), &resolved, &state);
    // `run_external` re-borrows the cell to glob the command name, so the
    // trace borrow must end first (RefCell, LESSONS.md).
    let builtin = cmd.builtin || child::dispatch::builtin_first(&cmd.name, &state);
    drop(state);

    if builtin {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::BorrowFailed)?;
        child::dispatch::run_builtin(cmd.name.clone(), &refs, args, &state)
    } else {
        external::run_external(&cmd, &refs, cell)
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
