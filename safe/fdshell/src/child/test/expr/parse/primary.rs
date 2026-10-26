//! The **primary** level of the `test` grammar: `( … )`, the unary form, the
//! binary-guarded form, `ARG OP ARG` and the single word. Extracted from
//! `child/test/expr/parse.rs` for the line budget (task #456); the connective
//! levels (`or`/`and`/`not`) stay in the parent.

use super::{next, parse_or, peek};
use crate::child::test::expr::Expr;
use crate::child::test::{fdsize, ops};
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, bail};

pub(super) fn parse_primary(expr: &[&CStr], pos: &mut usize) -> Result<Expr, Report<BuiltinError>> {
    if peek(expr, *pos).is_some_and(|t| t.to_bytes() == b"(") {
        return parse_group(expr, pos);
    }
    let Some(tok) = peek(expr, *pos) else {
        bail!(BuiltinError::TestUsage)
    };
    let b = tok.to_bytes();
    let lhs = next(expr, pos)?;
    if is_unary_tok(b) {
        let arg = next(expr, pos)?;
        return Ok(Expr::Unary { op: lhs, arg });
    }
    if ops::is_binary(b) {
        // A binary operator cannot start an expression.
        bail!(BuiltinError::TestUsage)
    }
    // A word followed by a binary operator is `ARG OP ARG`.
    if peek(expr, *pos).is_some_and(|t| ops::is_binary(t.to_bytes())) {
        let (op, rhs) = (next(expr, pos)?, next(expr, pos)?);
        return Ok(Expr::Binary { lhs, op, rhs });
    }
    Ok(Expr::Single(lhs))
}

fn parse_group(expr: &[&CStr], pos: &mut usize) -> Result<Expr, Report<BuiltinError>> {
    next(expr, pos)?; // the `(` we just matched
    let inner = parse_or(expr, pos)?;
    if peek(expr, *pos).is_none_or(|c| c.to_bytes() != b")") {
        bail!(BuiltinError::TestUsage)
    }
    next(expr, pos)?;
    Ok(inner)
}

/// A unary token: a file/string test or a `-fdsize±N` bound.
fn is_unary_tok(op: &[u8]) -> bool {
    ops::is_unary(op) || fdsize::parse(op).is_some()
}
