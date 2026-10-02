//! Expression layer: binary operators by precedence, unary prefixes, primaries.

use alloc::boxed::Box;
use error_stack::Report;

use super::ast::Ast;
use super::lex::Tok;
use super::op::Op;
use crate::error::resolve::ResolveError;

pub(super) type R = Result<Ast, Report<ResolveError>>;

/// `**` precedence; unary binds tighter (bash: `-2**2` = `(-2)**2` = 4).
const POW_PREC: u8 = 11;

pub(super) fn peek_op(toks: &[Tok], pos: usize) -> Option<Op> {
    match toks.get(pos) {
        Some(Tok::Op(op)) => Some(*op),
        _ => None,
    }
}

/// Binary-operator precedence (lower binds looser); `**` is right-associative.
fn precedence(op: Op) -> Option<u8> {
    Some(match op {
        Op::OrOr => 1,
        Op::AndAnd => 2,
        Op::Or => 3,
        Op::Xor => 4,
        Op::And => 5,
        Op::Eq | Op::Ne => 6,
        Op::Lt | Op::Le | Op::Gt | Op::Ge => 7,
        Op::Shl | Op::Shr => 8,
        Op::Plus | Op::Minus => 9,
        Op::Star | Op::Slash | Op::Percent => 10,
        Op::Pow => POW_PREC,
        _ => return None,
    })
}

pub(super) fn parse_expr(toks: &[Tok], pos: &mut usize, min_prec: u8) -> R {
    let mut lhs = super::primary::parse_unary(toks, pos)?;
    while let Some(op) = peek_op(toks, *pos) {
        let Some(prec) = precedence(op) else {
            break;
        };
        if prec < min_prec {
            break;
        }
        *pos += 1;
        let rhs_prec = if op == Op::Pow { prec } else { prec + 1 };
        let rhs = parse_expr(toks, pos, rhs_prec)?;
        lhs = Ast::Bin(op, Box::new(lhs), Box::new(rhs));
    }
    Ok(lhs)
}
