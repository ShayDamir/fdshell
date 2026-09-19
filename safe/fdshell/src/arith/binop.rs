//! Wrapping 64-bit arithmetic evaluation of `Bin` and `Assign` nodes,
//! matching bash's mod-2⁶⁴ semantics.

use error_stack::{Report, bail};

use super::ast::AssOp;
use super::op::Op;
use crate::error::resolve::ResolveError;

/// Evaluate a binary operator; comparisons yield 0/1.
pub(crate) fn binop(op: Op, l: i64, r: i64) -> Result<i64, Report<ResolveError>> {
    match op {
        Op::Plus => Ok(l.wrapping_add(r)),
        Op::Minus => Ok(l.wrapping_sub(r)),
        Op::Star => Ok(l.wrapping_mul(r)),
        Op::Slash => div_or_rem(Op::Slash, l, r),
        Op::Percent => div_or_rem(Op::Percent, l, r),
        Op::Shl => Ok(l.wrapping_shl(r as u32)),
        Op::Shr => Ok(l.wrapping_shr(r as u32)),
        Op::Lt => Ok(i64::from(l < r)),
        Op::Le => Ok(i64::from(l <= r)),
        Op::Gt => Ok(i64::from(l > r)),
        Op::Ge => Ok(i64::from(l >= r)),
        Op::Eq => Ok(i64::from(l == r)),
        Op::Ne => Ok(i64::from(l != r)),
        Op::And => Ok(l & r),
        Op::Or => Ok(l | r),
        Op::Xor => Ok(l ^ r),
        Op::Pow => Ok(powi(l, r)),
        // `&&`/`||` are short-circuited by the caller; `?`/`:` never appear
        // in `Bin` nodes.
        Op::AndAnd
        | Op::OrOr
        | Op::Question
        | Op::Colon
        | Op::Bang
        | Op::Tilde
        | Op::Assign
        | Op::AddAssign
        | Op::SubAssign
        | Op::MulAssign
        | Op::DivAssign
        | Op::RemAssign
        | Op::AndAssign
        | Op::OrAssign
        | Op::XorAssign
        | Op::ShlAssign
        | Op::ShrAssign => bail!(ResolveError::Never),
    }
}

/// `i64::MIN / -1` wraps (C/bash semantics), but a zero divisor is an error.
fn div_or_rem(op: Op, l: i64, r: i64) -> Result<i64, Report<ResolveError>> {
    if r == 0 {
        bail!(ResolveError::ArithDivZero);
    }
    Ok(if op == Op::Slash {
        l.wrapping_div(r)
    } else {
        l.wrapping_rem(r)
    })
}

/// `base ** exp` with wrapping multiplication; a negative exponent is 0
/// (the fractional result truncates), `0 ** 0` is 1.
fn powi(base: i64, exp: i64) -> i64 {
    if exp < 0 {
        return 0;
    }
    let mut result: i64 = 1;
    let mut b = base;
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result = result.wrapping_mul(b);
        }
        e >>= 1;
        if e > 0 {
            b = b.wrapping_mul(b);
        }
    }
    result
}

/// Compound assignment `cur op rhs`; `Set` discards `cur`.
pub(crate) fn apply(ass: AssOp, cur: i64, rhs: i64) -> Result<i64, Report<ResolveError>> {
    match ass {
        AssOp::Set => Ok(rhs),
        AssOp::Add => Ok(cur.wrapping_add(rhs)),
        AssOp::Sub => Ok(cur.wrapping_sub(rhs)),
        AssOp::Mul => Ok(cur.wrapping_mul(rhs)),
        AssOp::Div => div_or_rem(Op::Slash, cur, rhs),
        AssOp::Rem => div_or_rem(Op::Percent, cur, rhs),
        AssOp::And => Ok(cur & rhs),
        AssOp::Or => Ok(cur | rhs),
        AssOp::Xor => Ok(cur ^ rhs),
        AssOp::Shl => Ok(cur.wrapping_shl(rhs as u32)),
        AssOp::Shr => Ok(cur.wrapping_shr(rhs as u32)),
    }
}
