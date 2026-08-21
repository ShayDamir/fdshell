//! `test EXPR` / `[ EXPR ]` — bash-compatible expression tests.
//!
//! A single string is true when non-empty. File tests `-e -f -d` take a path
//! or a `%var` fd variable. String tests `= !=`. Integer tests
//! `-eq -ne -lt -le -gt -ge`. Malformed expressions exit 2.

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::ShortCStr;

use ops::is_unary;

pub(super) fn handle_test(
    name: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let (expr, orig) = if name.eq_bytes(b"[") {
        match refs.split_last() {
            Some((closer, body)) if closer.to_bytes() == b"]" => {
                // Drop the trailing `]` so `orig` stays parallel to `expr`.
                (body, args.get(..body.len()).unwrap_or(&[]))
            }
            _ => bail!(BuiltinError::TestMissingCloseBracket),
        }
    } else {
        (refs, args)
    };
    eval(expr, orig, state)
}

pub(super) fn eval(
    expr: &[&CStr],
    orig: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    // Zero operands is false, matching bash (`test` and `[ ]`).
    let (first, rest) = match expr.split_first() {
        Some(t) => t,
        None => return Ok(1),
    };
    match (first, rest) {
        // One operand: true iff the string is non-empty, even if it looks
        // like a unary operator (bash rule).
        (s, []) => Ok(usize::from(s.to_bytes().is_empty()) as i32),
        (op, [arg]) => {
            if !is_unary(op.to_bytes()) {
                bail!(BuiltinError::TestUsage);
            }
            // `arg` is `expr[1]`; its original token is `orig[1]` (the two
            // slices are parallel). A `%var` original means fd-table lookup.
            ops::file_test(op.to_bytes(), arg, orig.get(1), state)
        }
        (lhs, [op, rhs]) => ops::string_or_int_test(lhs, op, rhs),
        _ => bail!(BuiltinError::TestUsage),
    }
}

mod ops;

#[cfg(test)]
mod tests;
