//! `$((…))` arithmetic expansion: a small lex → parse → eval pipeline that
//! runs in-process during the substitution pass (no child, no `unsafe`).

mod ast;
mod binop;
mod eval;
mod expr;
mod lex;
mod lex_name;
mod lex_num;
mod lex_op;
mod op;
mod parse;
mod var;

use core::fmt::Write;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

/// Evaluate the body of a `$((…))` word, replacing it with the decimal result.
pub(crate) fn eval(body: &[u8], cell: &ForkCell<ShellState>) -> Result<i64, Report<ResolveError>> {
    eval_body(body, cell, 0)
}

/// Recursive entry: re-evaluate a variable's value as an expression; `depth`
/// counts the variable-reference chain (capped to catch `x=x`).
pub(crate) fn eval_body(
    body: &[u8],
    cell: &ForkCell<ShellState>,
    depth: u32,
) -> Result<i64, Report<ResolveError>> {
    let toks = lex::lex(body)?;
    let ast = parse::parse(&toks)?;
    eval::eval_ast(&ast, cell, depth)
}

/// Render a value as its decimal text.
pub(crate) fn render(value: i64) -> Result<ShortCStr, Report<ResolveError>> {
    let mut out = ShortCStr::new();
    core::write!(out, "{value}").change_context(ResolveError::Never)?;
    Ok(out)
}

#[cfg(test)]
mod tests;
