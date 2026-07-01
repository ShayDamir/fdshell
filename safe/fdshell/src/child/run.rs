use crate::child::{self, Command};
use crate::error::child_process::ChildProcessError;
use crate::exec;
use crate::redirect::Redirect;
use crate::resolve::substitute_args;
use crate::state::ShellState;
use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt, bail};
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

    // Clone exports before borrowing mutably
    let exports: Vec<(sys::ShortCStr, Vec<u8>)> = {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::BorrowFailed)?;
        state
            .exports
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };

    let state = cell
        .borrow_mut()
        .change_context(ChildProcessError::BorrowFailed)?;
    if cmd.builtin {
        let cmd_name = cmd.name.clone();
        match child::builtin::dispatch_builtin(cmd.name, &refs, args, &state) {
            Ok(code) => Ok(code),
            Err(report) => match *report.current_context() {
                BuiltinError::Unknown => bail!(ChildProcessError::NotABuiltin(cmd_name)),
                BuiltinError::Help => Ok(0),
                BuiltinError::InvalidArgument(_) => {
                    eprintln!("{:?}", report);
                    Ok(1)
                }
                BuiltinError::Io => {
                    Err(report.change_context(ChildProcessError::BuiltinExecutionFailed))
                }
                BuiltinError::Syscall => {
                    if let Some(e) = report.downcast_ref::<sys::SyscallError>() {
                        Ok(e.errno())
                    } else {
                        Ok(1)
                    }
                }
                BuiltinError::SendFdFailed => {
                    eprintln!("{:?}", report);
                    Ok(1)
                }
            },
        }
    } else {
        let name = sys::RefCStr::from(cmd.name.clone());
        let fd = exec::resolve_path(&name)
            .change_context(ChildProcessError::ResolveFailed(cmd.name.clone()))?;
        let full_argv: Vec<&CStr> = std::iter::once(name.as_ref())
            .chain(refs.iter().copied())
            .collect();
        match exec::exec_fd(&fd, &full_argv, &exports) {
            Ok(()) => Ok(0),
            Err(report) => Err(report.change_context(ChildProcessError::ExecFailed)),
        }
    }
}
