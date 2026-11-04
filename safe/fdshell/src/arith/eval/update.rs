//! The `++`/`--` side effect; the `eval` walk calls it on variable operands.

use super::super::ast::Ast;
use super::super::var;
use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use error_stack::Report;
use sys::fork_cell::ForkCell;

/// An arithmetic evaluation result.
type E = Result<i64, Report<ResolveError>>;

/// `++`/`--` on an operand: evaluate it, and write the new value back when the
/// operand is a variable. `pre` yields the new value, `post` the old one.
/// A non-variable operand is just evaluated with no side effect, which is
/// bash's rule (`++1` is `1`, `++(1+2)` is `3`).
pub(super) fn update(
    operand: &Ast,
    delta: i64,
    pre: bool,
    cell: &ForkCell<ShellState>,
    depth: u32,
) -> E {
    let old = super::eval_ast(operand, cell, depth)?;
    let new = old.wrapping_add(delta);
    if let Ast::Var(name) = operand {
        var::set_var(name, new, cell)?;
    }
    Ok(if pre { new } else { old })
}
