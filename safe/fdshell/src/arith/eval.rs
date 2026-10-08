//! AST evaluation: wrapping arithmetic, short-circuiting `&&`/`||` and the
//! ternary, `++`/`--` side effects, the comma operator, and variable
//! resolution in `var.rs`.

use error_stack::Report;

use super::ast::Ast;
use super::binop;
use super::op::Op;
use super::var;
use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

/// An arithmetic evaluation result.
type E = Result<i64, Report<ResolveError>>;

pub(crate) fn eval_ast(
    node: &Ast,
    cell: &ForkCell<ShellState>,
    depth: u32,
) -> Result<i64, Report<ResolveError>> {
    match node {
        Ast::Lit(n) => Ok(*n),
        Ast::Pid => {
            let state = crate::substitute::borrow_state(cell)?;
            Ok(i64::from(state.shell_pid.as_raw()))
        }
        Ast::LastBg => {
            let state = crate::substitute::borrow_state(cell)?;
            Ok(state.last_bg_pid.map_or(0, |pid| i64::from(pid.as_raw())))
        }
        Ast::Var(name) => var::eval_var(name, cell, depth),
        Ast::Neg(a) => Ok(eval_ast(a, cell, depth)?.wrapping_neg()),
        Ast::Pos(a) => eval_ast(a, cell, depth),
        Ast::Not(a) => Ok(i64::from(eval_ast(a, cell, depth)? == 0)),
        Ast::BitNot(a) => Ok(!eval_ast(a, cell, depth)?),
        Ast::PreInc(a) => update(a, 1, true, cell, depth),
        Ast::PostInc(a) => update(a, 1, false, cell, depth),
        Ast::PreDec(a) => update(a, -1, true, cell, depth),
        Ast::PostDec(a) => update(a, -1, false, cell, depth),
        // The comma evaluates its left operand for the side effect only and
        // yields the right operand; the left-to-right order is the same as `Bin`.
        Ast::Comma(l, r) => {
            eval_ast(l, cell, depth)?;
            eval_ast(r, cell, depth)
        }
        // `&&`/`||` are boolean like in C: the result is 0 or 1, not the
        // operand's value (bash: `1&&2` is 1, not 2).
        Ast::Bin(Op::AndAnd, l, r) => {
            let lv = eval_ast(l, cell, depth)?;
            if lv == 0 {
                Ok(0)
            } else {
                Ok(i64::from(eval_ast(r, cell, depth)? != 0))
            }
        }
        Ast::Bin(Op::OrOr, l, r) => {
            let lv = eval_ast(l, cell, depth)?;
            if lv != 0 {
                Ok(1)
            } else {
                Ok(i64::from(eval_ast(r, cell, depth)? != 0))
            }
        }
        Ast::Bin(op, l, r) => {
            binop::binop(*op, eval_ast(l, cell, depth)?, eval_ast(r, cell, depth)?)
        }
        Ast::Tern(c, t, e) => {
            if eval_ast(c, cell, depth)? != 0 {
                eval_ast(t, cell, depth)
            } else {
                eval_ast(e, cell, depth)
            }
        }
        Ast::Assign(ass, name, rhs) => {
            // The LHS value is read before the RHS is evaluated, so an
            // increment in the RHS sees bash's order (`x+=++x` is 3+4 = 7).
            let cur = var::eval_var(name, cell, depth)?;
            let rhs_v = eval_ast(rhs, cell, depth)?;
            let value = binop::apply(*ass, cur, rhs_v)?;
            var::set_var(name, value, cell)?;
            Ok(value)
        }
    }
}

/// `++`/`--` on an operand: evaluate it, and write the new value back when the
/// operand is a variable. `pre` yields the new value, `post` the old one.
/// A non-variable operand is just evaluated with no side effect, which is
/// bash's rule (`++1` is `1`, `++(1+2)` is `3`).
fn update(operand: &Ast, delta: i64, pre: bool, cell: &ForkCell<ShellState>, depth: u32) -> E {
    let old = eval_ast(operand, cell, depth)?;
    let new = old.wrapping_add(delta);
    if let Ast::Var(name) = operand {
        var::set_var(name, new, cell)?;
    }
    Ok(if pre { new } else { old })
}
