//! Operator lexing: 3-char ops before 2-char before 1-char (`<<=` vs `<<`).

use super::op::Op;

/// Lex the operator starting at `i`; returns the op and its byte length.
pub(crate) fn lex_op(body: &[u8], i: usize) -> Option<(Op, usize)> {
    if body.get(i..i + 3) == Some(b"<<=".as_slice()) {
        return Some((Op::ShlAssign, 3));
    }
    if body.get(i..i + 3) == Some(b">>=".as_slice()) {
        return Some((Op::ShrAssign, 3));
    }
    match body.get(i..i + 2) {
        Some(b"**") => return Some((Op::Pow, 2)),
        Some(b"<<") => return Some((Op::Shl, 2)),
        Some(b">>") => return Some((Op::Shr, 2)),
        Some(b"<=") => return Some((Op::Le, 2)),
        Some(b">=") => return Some((Op::Ge, 2)),
        Some(b"==") => return Some((Op::Eq, 2)),
        Some(b"!=") => return Some((Op::Ne, 2)),
        Some(b"&&") => return Some((Op::AndAnd, 2)),
        Some(b"||") => return Some((Op::OrOr, 2)),
        Some(b"+=") => return Some((Op::AddAssign, 2)),
        Some(b"-=") => return Some((Op::SubAssign, 2)),
        Some(b"*=") => return Some((Op::MulAssign, 2)),
        Some(b"/=") => return Some((Op::DivAssign, 2)),
        Some(b"%=") => return Some((Op::RemAssign, 2)),
        Some(b"&=") => return Some((Op::AndAssign, 2)),
        Some(b"|=") => return Some((Op::OrAssign, 2)),
        Some(b"^=") => return Some((Op::XorAssign, 2)),
        _ => {}
    }
    match body.get(i) {
        Some(b'+') => Some((Op::Plus, 1)),
        Some(b'-') => Some((Op::Minus, 1)),
        Some(b'*') => Some((Op::Star, 1)),
        Some(b'/') => Some((Op::Slash, 1)),
        Some(b'%') => Some((Op::Percent, 1)),
        Some(b'<') => Some((Op::Lt, 1)),
        Some(b'>') => Some((Op::Gt, 1)),
        Some(b'&') => Some((Op::And, 1)),
        Some(b'|') => Some((Op::Or, 1)),
        Some(b'^') => Some((Op::Xor, 1)),
        Some(b'!') => Some((Op::Bang, 1)),
        Some(b'~') => Some((Op::Tilde, 1)),
        Some(b'?') => Some((Op::Question, 1)),
        Some(b':') => Some((Op::Colon, 1)),
        Some(b'=') => Some((Op::Assign, 1)),
        _ => None,
    }
}
