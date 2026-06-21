#![forbid(unsafe_code)]

use crate::child;
use crate::exec;
use crate::resolve::substitute_args;
use crate::state::ShellState;
use std::ffi::CStr;
use sys::ShortCStr;
use sys::errno::EINVAL;
use sys::fork_cell::ForkCell;

pub fn replace_shell(
    args: &[ShortCStr],
    redirects: &[crate::redirect::RedirectDef],
    cell: &ForkCell<ShellState>,
) -> ! {
    let result = execute(args, redirects, cell);
    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => std::process::exit(e),
    }
}

fn execute(
    args: &[ShortCStr],
    redirects: &[crate::redirect::RedirectDef],
    cell: &ForkCell<ShellState>,
) -> Result<(), i32> {
    let opened = match crate::redirect::open_redirect_files(redirects) {
        Ok(o) => o,
        Err(_) => return Err(EINVAL),
    };
    let resolved = {
        let state = cell.borrow()?;
        match crate::redirect::resolve_redirects(redirects, &opened, &state) {
            Ok(r) => r,
            Err(_) => return Err(EINVAL),
        }
    };

    for r in &resolved {
        r.export()?;
    }

    sys::shellfd::set_capture_active(false);

    if args.first().is_some_and(|a| a.eq_bytes(b"builtin")) {
        let builtin_name = args.get(1).ok_or(sys::errno::EINVAL)?;
        let builtin_args = args.get(2..).unwrap_or(&[]);
        let substituted = match substitute_args(builtin_args, cell) {
            Ok(s) => s,
            Err(_) => return Err(EINVAL),
        };
        let refs: Vec<&CStr> = substituted.iter().map(|cs| cs.as_c_str()).collect();
        let state = cell.borrow()?;
        child::builtin::dispatch_builtin(builtin_name.clone(), &refs, builtin_args, &state)
    } else {
        let binary = args.first().ok_or(sys::errno::EINVAL)?;
        let binary = sys::RefCStr::from(binary.clone());
        let fd = exec::resolve_path(&binary)?;
        let substituted = match substitute_args(args.get(1..).unwrap_or(&[]), cell) {
            Ok(s) => s,
            Err(_) => return Err(EINVAL),
        };
        let argv: Vec<&CStr> = std::iter::once(binary.as_ref())
            .chain(substituted.iter().map(|s| s.as_c_str()))
            .collect();
        let state = cell.borrow()?;
        let exports: Vec<(ShortCStr, Vec<u8>)> = state
            .exports
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        exec::exec_fd(&fd, &argv, &exports)
    }
}
