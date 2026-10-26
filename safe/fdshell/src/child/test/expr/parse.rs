//! Recursive-descent parser for `test` expressions.
//!
//! Grammar, loosest to tightest: `or := and ( '-o' and )*`,
//! `and := not ( '-a' not )*`, `not := '!' not | primary`,
//! `primary := '(' or ')' | unary | binary | single`. A missing operand, an
//! unclosed `(`, or a trailing token is `TestUsage`.

use alloc::boxed::Box;

use core::ffi::CStr;
use error_stack::{Report, bail};

use builtins::error::BuiltinError;

mod primary;

use super::Expr;
use primary::parse_primary;

pub(super) fn parse(expr: &[&CStr]) -> Result<Expr, Report<BuiltinError>> {
    // Bash's single-argument rule: true iff the string is non-empty, even for
    // a word that looks like an operator (`test -f` → true).
    if expr.len() == 1 {
        return Ok(Expr::Single(0));
    }
    let mut pos = 0;
    let ast = parse_or(expr, &mut pos)?;
    // A token the grammar did not consume is a malformed expression.
    if expr.get(pos).is_some() {
        bail!(BuiltinError::TestUsage)
    }
    Ok(ast)
}

pub(super) fn peek<'a>(expr: &'a [&'a CStr], pos: usize) -> Option<&'a CStr> {
    expr.get(pos).copied()
}

/// Consume the token at `pos`, returning its index.
pub(super) fn next(expr: &[&CStr], pos: &mut usize) -> Result<usize, Report<BuiltinError>> {
    let i = *pos;
    peek(expr, i).ok_or(BuiltinError::TestUsage)?;
    *pos += 1;
    Ok(i)
}

pub(super) fn parse_or(expr: &[&CStr], pos: &mut usize) -> Result<Expr, Report<BuiltinError>> {
    let mut lhs = parse_and(expr, pos)?;
    while peek(expr, *pos).is_some_and(|t| t.to_bytes() == b"-o") {
        *pos += 1;
        let rhs = parse_and(expr, pos)?;
        lhs = Expr::Or(Box::new(lhs), Box::new(rhs));
    }
    Ok(lhs)
}

fn parse_and(expr: &[&CStr], pos: &mut usize) -> Result<Expr, Report<BuiltinError>> {
    let mut lhs = parse_not(expr, pos)?;
    while peek(expr, *pos).is_some_and(|t| t.to_bytes() == b"-a") {
        *pos += 1;
        let rhs = parse_not(expr, pos)?;
        lhs = Expr::And(Box::new(lhs), Box::new(rhs));
    }
    Ok(lhs)
}

fn parse_not(expr: &[&CStr], pos: &mut usize) -> Result<Expr, Report<BuiltinError>> {
    if peek(expr, *pos).is_some_and(|t| t.to_bytes() == b"!") {
        *pos += 1;
        return Ok(Expr::Not(Box::new(parse_not(expr, pos)?)));
    }
    parse_primary(expr, pos)
}
