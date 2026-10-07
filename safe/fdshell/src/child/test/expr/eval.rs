//! Evaluator for parsed `test` expressions. Logical nodes evaluate **both**
//! children (bash's `test` reports an error from either side of `-a`/`-o`,
//! never short-circuiting); primaries reuse the existing operator helpers.

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::Report;
use sys::ShortCStr;

use super::super::fdsize;
use super::super::filetest::{file_binary_test, file_test};
use super::super::ops::{is_file_binary, string_or_int_test, string_test};
use super::Expr;

pub(super) fn eval(
    ast: &Expr,
    expr: &[&CStr],
    orig: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    match ast {
        Expr::Single(i) => {
            let s = operand(expr, *i)?;
            Ok(usize::from(s.to_bytes().is_empty()) as i32)
        }
        Expr::Unary { op, arg } => unary(expr, orig, *op, *arg, state),
        Expr::Binary { lhs, op, rhs } => binary(expr, orig, *lhs, *op, *rhs, state),
        Expr::Not(e) => Ok(usize::from(eval(e, expr, orig, state)? == 0) as i32),
        Expr::And(a, b) => {
            let l = eval(a, expr, orig, state)?;
            let r = eval(b, expr, orig, state)?;
            Ok(usize::from(l != 0 || r != 0) as i32)
        }
        Expr::Or(a, b) => {
            let l = eval(a, expr, orig, state)?;
            let r = eval(b, expr, orig, state)?;
            Ok(usize::from(l != 0 && r != 0) as i32)
        }
    }
}

/// The token at `i`; the parser validated every index it produced.
fn operand<'a>(expr: &'a [&'a CStr], i: usize) -> Result<&'a CStr, Report<BuiltinError>> {
    Ok(expr.get(i).copied().ok_or(BuiltinError::Never)?)
}

fn unary(
    expr: &[&CStr],
    orig: &[ShortCStr],
    op_idx: usize,
    arg_idx: usize,
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let (op, arg) = (operand(expr, op_idx)?, operand(expr, arg_idx)?);
    let ob = op.to_bytes();
    if let Some(spec) = fdsize::parse(ob) {
        return fdsize::test(spec, arg, orig.get(arg_idx), state);
    }
    if ob == b"-z" || ob == b"-n" {
        return string_test(ob, arg);
    }
    file_test(ob, arg, orig.get(arg_idx), state)
}

fn binary(
    expr: &[&CStr],
    orig: &[ShortCStr],
    lhs: usize,
    op: usize,
    rhs: usize,
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let (l, o, r) = (operand(expr, lhs)?, operand(expr, op)?, operand(expr, rhs)?);
    if is_file_binary(o.to_bytes()) {
        file_binary_test(l, o.to_bytes(), orig.get(lhs), r, orig.get(rhs), state)
    } else {
        string_or_int_test(l, o, r)
    }
}
