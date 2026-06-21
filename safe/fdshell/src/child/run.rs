use crate::child::{self, Command};
use crate::error::child::ChildError;
use crate::exec;
use crate::redirect::Redirect;
use crate::resolve::substitute_args;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use std::ffi::CStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub fn child_main(
    child_sock: Option<sys::LocalFd>,
    cell: &ForkCell<ShellState>,
    cmd: Command,
    args: &[ShortCStr],
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

    let resolved = substitute_args(args, cell).change_context(ChildError::SubstituteFailed)?;
    let refs: Vec<&CStr> = resolved.iter().map(|cs| cs.as_c_str()).collect();

    let state = cell.borrow_mut().change_context(ChildError::BorrowFailed)?;
    match cmd {
        Command::Builtin(name) => {
            let cmd_name = name.clone();
            match child::builtin::dispatch_builtin(name, &refs, args, &state) {
                Ok(()) => Ok(0),
                Err(code) if code == sys::errno::ENOSYS => {
                    Err(Report::new(ChildError::NotABuiltin).attach(cmd_name))
                }
                Err(code) => Ok(code),
            }
        }
        Command::External(name) => {
            let name = sys::RefCStr::from(name.clone());
            let fd = exec::resolve_path(&name).change_context(ChildError::NotFound)?;
            let full_argv: Vec<&CStr> = std::iter::once(name.as_ref())
                .chain(refs.iter().copied())
                .collect();
            let exports: Vec<(sys::ShortCStr, Vec<u8>)> = state
                .exports
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            match exec::exec_fd(&fd, &full_argv, &exports) {
                Ok(()) => Ok(0),
                Err(report) => Err(report.change_context(ChildError::ExecFailed)),
            }
        }
    }
}
