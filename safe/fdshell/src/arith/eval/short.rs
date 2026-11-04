//! The short-circuiting `&&`/`||` arms; one algorithm, two operators.

use super::super::ast::Ast;
use super::super::op::Op;
use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use error_stack::Report;
use sys::fork_cell::ForkCell;

/// An arithmetic evaluation result.
type E = Result<i64, Report<ResolveError>>;

/// `&&`/`||` are boolean like in C: the result is 0 or 1, not the operand's
/// value (bash: `1&&2` is 1, not 2). The left operand is evaluated first, and
/// the right one only when the left one's truth does not decide the result:
/// falsy short-circuits `&&` to 0, truthy short-circuits `||` to 1.
pub(super) fn short_circuit(
    op: Op,
    l: &Ast,
    r: &Ast,
    cell: &ForkCell<ShellState>,
    depth: u32,
) -> E {
    let truth = super::eval_ast(l, cell, depth)? != 0;
    let is_or = op == Op::OrOr;
    if truth == is_or {
        return Ok(i64::from(is_or));
    }
    Ok(i64::from(super::eval_ast(r, cell, depth)? != 0))
}
