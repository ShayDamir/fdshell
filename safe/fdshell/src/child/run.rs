use crate::child::{self, Command};
use crate::error::child::ChildError;
use crate::exec;
use crate::redirect::Redirect;
use crate::resolve::substitute_args;
use crate::state::ShellState;
use builtins::error::BuiltinError;
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
) -> Result<i32, Report<ChildError>> {
    if let Some(sock) = child_sock {
        sock.export_to(sys::shellfd::SHELLFD)
            .change_context(ChildError::RedirectFailed)?;
        sys::shellfd::set_capture_active(true);
    } else {
        sys::shellfd::set_capture_active(false);
    }

    for r in redirects {
        r.export().change_context(ChildError::RedirectFailed)?;
    }

    let resolved =
        substitute_args(args, args_fq, cell).change_context(ChildError::SubstituteFailed)?;
    let refs: Vec<&CStr> = resolved.iter().map(|cs| cs.as_c_str()).collect();

    // Clone exports before borrowing mutably
    let exports: Vec<(sys::ShortCStr, Vec<u8>)> = {
        let state = cell.borrow().change_context(ChildError::BorrowFailed)?;
        state
            .exports
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };

    let state = cell.borrow_mut().change_context(ChildError::BorrowFailed)?;
    if cmd.builtin {
        let cmd_name = cmd.name.clone();
        match child::builtin::dispatch_builtin(cmd.name, &refs, args, &state) {
            Ok(code) => Ok(code),
            Err(report) => match *report.current_context() {
                BuiltinError::Unknown => Err(Report::new(ChildError::NotABuiltin).attach(cmd_name)),
                BuiltinError::Help => Ok(0),
               BuiltinError::InvalidArgument => Ok(1),
                BuiltinError::Io => Err(report.change_context(ChildError::Io)),
                BuiltinError::Syscall(e) => Ok(e.errno()),
            },
        }
    } else {
        let name = sys::RefCStr::from(cmd.name.clone());
        let fd = exec::resolve_path(&name).change_context(ChildError::NotFound)?;
        let full_argv: Vec<&CStr> = std::iter::once(name.as_ref())
            .chain(refs.iter().copied())
            .collect();
        match exec::exec_fd(&fd, &full_argv, &exports) {
            Ok(()) => Ok(0),
            Err(report) => Err(report.change_context(ChildError::ExecFailed)),
        }
    }
}
