//! `test EXPR` / `[ EXPR ]` — bash-compatible expression tests.
//!
//! A single string is true when non-empty. File tests take a path or a `%var`
//! fd variable: kinds, size, size bounds (`-fdsize+N` / `-fdsize-N`,
//! fdshell-only), mode bits, tty, permissions (`-e -f -d -b -c -p -S -L -s
//! -g -k -t -r -w -x`) and binary comparisons (`-nt -ot -ef -fdeq -fdne`).
//! String tests `= !=` and `-z -n`. Integer tests `-eq -ne -lt -le -gt -ge`.
//! Logical operators `-a`, `-o`, `!` and `( )` grouping combine the
//! primaries (see `expr`). Malformed expressions exit 2.

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::ShortCStr;

use super::Ctx;

pub(super) fn handle_test(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let (expr, orig) = if ctx.name.eq_bytes(b"[") {
        match ctx.refs.split_last() {
            Some((closer, body)) if closer.to_bytes() == b"]" => {
                // Drop the trailing `]` so `orig` stays parallel to `expr`.
                (body, ctx.args.get(..body.len()).unwrap_or(&[]))
            }
            _ => bail!(BuiltinError::TestMissingCloseBracket),
        }
    } else {
        (ctx.refs, ctx.args)
    };
    eval(expr, orig, ctx.state)
}

pub(super) fn eval(
    expr: &[&CStr],
    orig: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    expr::eval_expr(expr, orig, state)
}

mod expr;
mod fdsize;
mod filetest;
mod ops;
mod perm;
mod stat;

#[cfg(test)]
mod tests;
