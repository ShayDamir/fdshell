mod external;

use crate::bytes::fold::fold_word;
use crate::child;
use crate::error::child_process::ChildProcessError;
use crate::state::ShellState;
use crate::substitute::substitute_args;
use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub fn execute(
    args: &[ShortCStr],
    args_mask: &[Vec<bool>],
    args_quoted: &[bool],
    redirects: &[crate::redirect::RedirectDef],
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<ChildProcessError>> {
    let opened = crate::redirect::open_redirect_files(redirects, cell)
        .change_context(ChildProcessError::ExportFailed)?;
    let resolved = crate::redirect::resolve_redirects(redirects, &opened, cell)
        .change_context(ChildProcessError::ExportFailed)?;

    for r in &resolved {
        r.export().change_context(ChildProcessError::ExportFailed)?;
    }

    sys::shellfd::set_capture_active(false);

    if args.first().is_some_and(|a| a.eq_bytes(b"builtin")) {
        let raw_name = args.get(1).ok_or(ChildProcessError::MissingArg)?;
        // The builtin name is raw word text, never substituted: fold its
        // unquoted escape pairs (POSIX #4.1) before the dispatch-table match.
        let (builtin_name, _) =
            fold_word(raw_name, args_mask.get(1).map(Vec::as_slice).unwrap_or(&[]))
                .change_context(ChildProcessError::ExecFailed)?;
        let builtin_args = args.get(2..).unwrap_or(&[]);
        let substituted = substitute_args(
            builtin_args,
            args_mask.get(2..).unwrap_or(&[]),
            args_quoted.get(2..).unwrap_or(&[]),
            cell,
        )
        .change_context(ChildProcessError::ExecFailed)?;
        let sealed: Vec<sys::ExportedCStr> = substituted.iter().map(|cs| cs.export()).collect();
        let refs: Vec<&CStr> = sealed.iter().map(|rc| rc.as_ref()).collect();
        let state = cell
            .borrow()
            .change_context(ChildProcessError::ExecFailed)?;
        crate::xtrace::trace(builtin_name.as_bytes().unwrap_or(&[]), &substituted, &state);
        child::dispatch::run_builtin(builtin_name.clone(), &refs, builtin_args, &state)
    } else {
        external::run(args, args_mask, args_quoted, cell)
    }
}
