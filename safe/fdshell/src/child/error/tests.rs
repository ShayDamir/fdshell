#![allow(clippy::unwrap_used)]
use builtins::error::BuiltinError;
use error_stack::Report;
use sys::ShortCStr;

use super::handle_builtin_error;

#[test]
fn handle_builtin_error_help_returns_zero() {
    let report = Report::new(BuiltinError::Help);
    assert!(matches!(
        handle_builtin_error(ShortCStr::from(c"help"), report),
        Ok(0)
    ));
}

#[test]
fn handle_builtin_error_unknown_is_err() {
    let report = Report::new(BuiltinError::Unknown);
    assert!(handle_builtin_error(ShortCStr::from(c"nope"), report).is_err());
}
