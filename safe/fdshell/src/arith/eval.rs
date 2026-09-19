//! AST evaluation: wrapping arithmetic, short-circuiting `&&`/`||` and the
//! ternary, variable resolution in `var.rs`.

use error_stack::Report;

use super::ast::Ast;
use super::binop;
use super::op::Op;
use super::var;
use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

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
            let rhs_v = eval_ast(rhs, cell, depth)?;
            let cur = var::eval_var(name, cell, depth)?;
            let value = binop::apply(*ass, cur, rhs_v)?;
            var::set_var(name, value, cell)?;
            Ok(value)
        }
    }
}
