#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec::Vec;

use builtins::error::BuiltinError;
use sys::ShortCStr;

use crate::state::ShellState;

use super::handle_copy_file_range;

fn with_refs<R, F>(args: &[&str], f: F) -> R
where
    F: FnOnce(&[&core::ffi::CStr], &[ShortCStr]) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let origs: Vec<ShortCStr> = args
        .iter()
        .map(|a| ShortCStr::from_vec(a.as_bytes().to_vec()).unwrap())
        .collect();
    f(&refs, &origs)
}

/// A fresh shell state with no fd vars; used for error-path assertions that
/// need no backing fd (avoiding `memfd_create`, which is unavailable in this
/// dev container).
fn empty_state() -> ShellState {
    ShellState::new()
}

#[test]
fn handle_missing_args_propagates_missing_argument() {
    let state = empty_state();
    with_refs(&[], |refs, origs| {
        let e = handle_copy_file_range(c"copy_file_range".into(), refs, origs, &state).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::MissingArgument("in fd var")
        ));
    });
}

#[test]
fn handle_unset_var_is_fdvar_not_found() {
    let state = empty_state();
    with_refs(&["%in", "%out"], |refs, origs| {
        let e = handle_copy_file_range(c"copy_file_range".into(), refs, origs, &state).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::FdVarNotFound));
    });
}
