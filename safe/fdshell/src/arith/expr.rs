//! Expression layer: binary operators by precedence, unary prefixes, primaries.

use alloc::boxed::Box;
use error_stack::{Report, bail};

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
    let mut lhs = parse_unary(toks, pos)?;
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

fn parse_unary(toks: &[Tok], pos: &mut usize) -> R {
    // The operand parses above `**`, so `-2**2` is `(-2)**2` = 4 (bash).
    let operand_prec = POW_PREC + 1;
    let make: Option<fn(Box<Ast>) -> Ast> = match peek_op(toks, *pos) {
        Some(Op::Minus) => Some(Ast::Neg),
        Some(Op::Plus) => Some(Ast::Pos),
        Some(Op::Bang) => Some(Ast::Not),
        Some(Op::Tilde) => Some(Ast::BitNot),
        _ => None,
    };
    let Some(make) = make else {
        return parse_primary(toks, pos);
    };
    *pos += 1;
    Ok(make(Box::new(parse_expr(toks, pos, operand_prec)?)))
}

fn parse_primary(toks: &[Tok], pos: &mut usize) -> R {
    let Some(tok) = toks.get(*pos).cloned() else {
        bail!(ResolveError::ArithSyntax);
    };
    *pos += 1;
    match tok {
        Tok::Num(n) => Ok(Ast::Lit(n)),
        Tok::Name(n) => Ok(Ast::Var(n)),
        Tok::Pid => Ok(Ast::Pid),
        Tok::LastBg => Ok(Ast::LastBg),
        Tok::LParen => {
            let inner = super::parse::parse_assign(toks, pos)?;
            match toks.get(*pos) {
                Some(Tok::RParen) => {
                    *pos += 1;
                    Ok(inner)
                }
                _ => bail!(ResolveError::ArithSyntax),
            }
        }
        Tok::RParen | Tok::Op(_) => bail!(ResolveError::ArithSyntax),
    }
}
