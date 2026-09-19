use super::ast::AssOp;

/// Every operator token of a `$((…))` expression; precedence and
/// associativity are fixed in `parse.rs`.
#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub(crate) enum Op {
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Pow,
    Shl,
    Shr,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
    Xor,
    AndAnd,
    OrOr,
    Question,
    Colon,
    Bang,
    Tilde,
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    RemAssign,
    AndAssign,
    OrAssign,
    XorAssign,
    ShlAssign,
    ShrAssign,
}

impl Op {
    /// Assignment operators of `name op= rhs`; `None` for every other op.
    pub(crate) fn as_assign(self) -> Option<AssOp> {
        match self {
            Op::Assign => Some(AssOp::Set),
            Op::AddAssign => Some(AssOp::Add),
            Op::SubAssign => Some(AssOp::Sub),
            Op::MulAssign => Some(AssOp::Mul),
            Op::DivAssign => Some(AssOp::Div),
            Op::RemAssign => Some(AssOp::Rem),
            Op::AndAssign => Some(AssOp::And),
            Op::OrAssign => Some(AssOp::Or),
            Op::XorAssign => Some(AssOp::Xor),
            Op::ShlAssign => Some(AssOp::Shl),
            Op::ShrAssign => Some(AssOp::Shr),
            _ => None,
        }
    }
}
