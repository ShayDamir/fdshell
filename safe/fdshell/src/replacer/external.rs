//! The non-`builtin` branch of `replacer::execute`: the `builtin_first`
//! option dispatch, then external commands (hash-table aware).

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

pub(super) fn run(
    args: &[ShortCStr],
    args_mask: &[Vec<bool>],
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<ChildProcessError>> {
    let binary = args.first().ok_or(ChildProcessError::MissingArg)?;
    let is_builtin = {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::ExecFailed)?;
        child::dispatch::builtin_first(binary, &state)
    };
    if is_builtin {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::ExecFailed)?;
        let rest = args.get(1..).unwrap_or(&[]);
        let substituted =
            substitute_args(rest, args_mask, cell).change_context(ChildProcessError::ExecFailed)?;
        let sealed: Vec<sys::ExportedCStr> = substituted.iter().map(|cs| cs.export()).collect();
        let refs: Vec<&CStr> = sealed.iter().map(|rc| rc.as_ref()).collect();
        crate::xtrace::trace(binary.as_bytes().unwrap_or(&[]), &substituted, &state);
        return match child::dispatch::dispatch_builtin(binary.clone(), &refs, rest, &state) {
            Ok(code) => Ok(code),
            Err(report) => crate::child::handle_builtin_error(binary.clone(), report),
        };
    }
    // No caching here: `become`/`exec` exit the shell right after, so the
    // table would never survive — the lookup still honors existing pins.
    let fd = {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::ExecFailed)?;
        exec::resolve_path(binary, &state.hash_table)
            .change_context(ChildProcessError::ExecFailed)?
    };
    let binary_exported = binary.export();
    let binary_cstr = binary_exported.as_ref();
    let substituted = substitute_args(args.get(1..).unwrap_or(&[]), args_mask, cell)
        .change_context(ChildProcessError::ExecFailed)?;
    let sealed: Vec<sys::ExportedCStr> = substituted.iter().map(|cs| cs.export()).collect();
    let mut argv: Vec<&CStr> = alloc::vec![binary_cstr];
    for s in &sealed {
        argv.push(s.as_ref());
    }
    let state = cell
        .borrow()
        .change_context(ChildProcessError::ExecFailed)?;
    crate::xtrace::trace(binary.as_bytes().unwrap_or(&[]), &substituted, &state);
    match exec::exec_fd(
        &fd,
        &argv,
        &state.environ,
        &state.exports,
        &state.env_filter,
        None,
    ) {
        Ok(()) => Ok(0),
        Err(report) => Err(report),
    }
}
