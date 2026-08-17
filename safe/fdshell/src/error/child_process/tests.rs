#![allow(clippy::unwrap_used)]
use super::*;

#[test]
fn not_found_exit_code_is_127() {
    let e = ChildProcessError::NotFound(ShortCStr::from(c"missing"));
    assert_eq!(e.exit_code(), 127);
}

#[test]
fn other_errors_exit_code_is_1() {
    let e = ChildProcessError::ExecFailed;
    assert_eq!(e.exit_code(), 1);
}
