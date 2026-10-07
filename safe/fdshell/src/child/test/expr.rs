//! `test`/`[` expressions: a recursive-descent parser and evaluator over the
//! token slices. Logical operators (`-o`, `-a`, `!`, `( )` grouping) combine
//! the existing primaries; the AST is index-based (token positions), so the
//! evaluator reuses the parallel `refs`/`orig` slices.

use alloc::boxed::Box;

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::Report;
use sys::ShortCStr;

/// A parsed `test` expression; every operand position is a token index.
#[derive(Debug, PartialEq)]
pub(super) enum Expr {
    /// A single string: true iff non-empty.
    Single(usize),
    /// `OP ARG`: a unary operator (file/string test, `-fdsize±N`).
    Unary { op: usize, arg: usize },
    /// `ARG OP ARG`: a binary operator (string/int/file comparison).
    Binary { lhs: usize, op: usize, rhs: usize },
    /// `! EXPR`.
    Not(Box<Expr>),
    /// `EXPR -a EXPR`; both sides are always evaluated.
    And(Box<Expr>, Box<Expr>),
    /// `EXPR -o EXPR`; both sides are always evaluated.
    Or(Box<Expr>, Box<Expr>),
}

/// Evaluate a `test` expression; zero operands is false (bash rule).
pub(super) fn eval_expr(
    expr: &[&CStr],
    orig: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    if expr.is_empty() {
        return Ok(1);
    }
    let ast = parse::parse(expr)?;
    eval::eval(&ast, expr, orig, state)
}

mod eval;
mod parse;

#[cfg(test)]
mod tests;
