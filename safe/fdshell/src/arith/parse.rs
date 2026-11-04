//! Parser entry point: an expression (see `expr.rs`) with assignment and
//! ternary on top (both right-associative, lowest precedence), and the comma
//! operator above them (left-associative, lowest of all).

use alloc::boxed::Box;
use error_stack::bail;

use super::ast::Ast;
use super::expr::{self, R};
use super::lex::Tok;
use super::op::Op;
use crate::error::resolve::ResolveError;

pub(crate) fn parse(toks: &[Tok], depth: u32) -> R {
    if toks.is_empty() {
        bail!(ResolveError::ArithSyntax);
    }
    let mut pos = 0usize;
    let ast = parse_comma(toks, &mut pos, depth)?;
    if pos != toks.len() {
        bail!(ResolveError::ArithSyntax);
    }
    Ok(ast)
}

/// The comma operator: the value is the last operand, the earlier ones are
/// evaluated for their side effects (bash: `1,2,3` is `3`).
pub(crate) fn parse_comma(toks: &[Tok], pos: &mut usize, depth: u32) -> R {
    let mut lhs = parse_assign(toks, pos, depth)?;
    while expr::peek_op(toks, *pos) == Some(Op::Comma) {
        *pos += 1;
        let rhs = parse_assign(toks, pos, depth)?;
        lhs = Ast::Comma(Box::new(lhs), Box::new(rhs));
    }
    Ok(lhs)
}

pub(crate) fn parse_assign(toks: &[Tok], pos: &mut usize, depth: u32) -> R {
    let lhs = parse_ternary(toks, pos, depth)?;
    let Some(op) = expr::peek_op(toks, *pos) else {
        return Ok(lhs);
    };
    let Some(ass) = op.as_assign() else {
        return Ok(lhs);
    };
    *pos += 1;
    let rhs = parse_assign(toks, pos, depth + 1)?;
    let name = lhs.var_name()?;
    Ok(Ast::Assign(ass, name, Box::new(rhs)))
}

fn parse_ternary(toks: &[Tok], pos: &mut usize, depth: u32) -> R {
    let cond = expr::parse_expr(toks, pos, 1, depth)?;
    if expr::peek_op(toks, *pos) != Some(Op::Question) {
        return Ok(cond);
    }
    *pos += 1;
    let then = parse_assign(toks, pos, depth + 1)?;
    if expr::peek_op(toks, *pos) != Some(Op::Colon) {
        bail!(ResolveError::ArithSyntax);
    }
    *pos += 1;
    let els = parse_ternary(toks, pos, depth + 1)?;
    Ok(Ast::Tern(Box::new(cond), Box::new(then), Box::new(els)))
}
