//! Arithmetic primaries: unary prefixes and the leaf tokens.

use alloc::boxed::Box;
use error_stack::bail;

use super::ast::Ast;
use super::expr::{R, parse_expr, peek_op};
use super::lex::Tok;
use super::op::Op;
use crate::error::resolve::ResolveError;

/// The `**` precedence; unary binds tighter (bash: `-2**2` = `(-2)**2` = 4).
const POW_PREC: u8 = 11;

pub(super) fn parse_unary(toks: &[Tok], pos: &mut usize) -> R {
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
