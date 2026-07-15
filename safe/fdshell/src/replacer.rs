#![allow(clippy::indexing_slicing)]
use crate::child;
use crate::error::child_process::ChildProcessError;
use crate::exec;
use crate::state::ShellState;
use crate::substitute::substitute_args;
use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub fn execute(
    args: &[ShortCStr],
    args_fq: &[bool],
    redirects: &[crate::redirect::RedirectDef],
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<ChildProcessError>> {
    let opened = crate::redirect::open_redirect_files(redirects)
        .change_context(ChildProcessError::ExportFailed)?;
    let resolved = {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::ExportFailed)?;
        crate::redirect::resolve_redirects(redirects, &opened, &state)
            .change_context(ChildProcessError::ExportFailed)?
    };

    for r in &resolved {
        r.export().change_context(ChildProcessError::ExportFailed)?;
    }

    sys::shellfd::set_capture_active(false);

    if args.first().is_some_and(|a| a.eq_bytes(b"builtin")) {
        let builtin_name = args.get(1).ok_or(ChildProcessError::MissingArg)?;
        let builtin_args = args.get(2..).unwrap_or(&[]);
        let substituted = substitute_args(builtin_args, &[], cell)
            .change_context(ChildProcessError::ExecFailed)?;
        let refs: Vec<&CStr> = substituted.iter().map(|cs| cs.as_c_str()).collect();
        let state = cell
            .borrow()
            .change_context(ChildProcessError::ExecFailed)?;
        match child::dispatch::dispatch_builtin(builtin_name.clone(), &refs, builtin_args, &state) {
            Ok(code) => Ok(code),
            Err(report) => crate::child::handle_builtin_error(builtin_name.clone(), report),
        }
    } else {
        let binary = args.first().ok_or(ChildProcessError::MissingArg)?;
        let binary_ref = sys::RefCStr::from(binary.clone());
        let fd = exec::resolve_path(&binary_ref).change_context(ChildProcessError::ExecFailed)?;
        let binary_cstr =
            core::ffi::CStr::from_bytes_with_nul(binary_ref.as_ref().to_bytes_with_nul())
                .change_context(ChildProcessError::Never)?;
        let substituted = substitute_args(args.get(1..).unwrap_or(&[]), args_fq, cell)
            .change_context(ChildProcessError::ExecFailed)?;
        let mut argv: Vec<&CStr> = alloc::vec![binary_cstr];
        for s in &substituted {
            argv.push(s.as_c_str());
        }
        let state = cell
            .borrow()
            .change_context(ChildProcessError::ExecFailed)?;
        match exec::exec_fd(&fd, &argv, &state.exports, &state.env_filter, None) {
            Ok(()) => Ok(0),
            Err(report) => Err(report),
        }
    }
}
