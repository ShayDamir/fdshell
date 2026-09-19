use alloc::boxed::Box;
use error_stack::{Report, bail};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;

use super::op::Op;

/// Compound-assignment operations of `Assign` AST nodes.
#[derive(Clone, Copy)]
pub(crate) enum AssOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
    Shl,
    Shr,
}

/// One node of a `$((…))` expression.
#[derive(Clone)]
pub(crate) enum Ast {
    Lit(i64),
    Var(ShortCStr),
    Pid,
    LastBg,
    Neg(Box<Ast>),
    Pos(Box<Ast>),
    Not(Box<Ast>),
    BitNot(Box<Ast>),
    Bin(Op, Box<Ast>, Box<Ast>),
    Tern(Box<Ast>, Box<Ast>, Box<Ast>),
    Assign(AssOp, ShortCStr, Box<Ast>),
}

impl Ast {
    /// The left side of an assignment must be a plain variable name.
    pub(crate) fn var_name(self) -> Result<ShortCStr, Report<ResolveError>> {
        match self {
            Ast::Var(name) => Ok(name),
            _ => bail!(ResolveError::ArithSyntax),
        }
    }
}
