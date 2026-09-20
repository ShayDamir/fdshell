//! `test EXPR` / `[ EXPR ]` — bash-compatible expression tests.
//!
//! A single string is true when non-empty. File tests take a path or a `%var`
//! fd variable: kinds, size, mode bits, tty, permissions (`-e -f -d -b -c -p
//! -S -L -s -g -k -t -r -w -x`) and binary comparisons (`-nt -ot -ef -fdeq
//! -fdne`). String tests `= !=` and `-z -n`. Integer tests `-eq -ne -lt -le
//! -gt -ge`. Malformed expressions exit 2.

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::ShortCStr;

use filetest::{file_binary_test, file_test};
use ops::{is_file_binary, is_unary, string_or_int_test, string_test};

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
            let op = op.to_bytes();
            if !is_unary(op) {
                bail!(BuiltinError::TestUsage);
            }
            if op == b"-z" || op == b"-n" {
                return string_test(op, arg);
            }
            // `arg` is `expr[1]`; its original token is `orig[1]` (the two
            // slices are parallel). A `%var` original means fd-table lookup.
            file_test(op, arg, orig.get(1), state)
        }
        (lhs, [op, rhs]) => {
            let opb = op.to_bytes();
            if is_file_binary(opb) {
                file_binary_test(lhs, opb, orig.first(), rhs, orig.get(2), state)
            } else {
                string_or_int_test(lhs, op, rhs)
            }
        }
        _ => bail!(BuiltinError::TestUsage),
    }
}

mod filetest;
mod ops;
mod perm;
mod stat;

#[cfg(test)]
mod tests;
