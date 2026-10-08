//! Operator lexing: 3-char ops before 2-char before 1-char (`<<=` vs `<<`).

use super::op::Op;

/// Lex the operator starting at `i`; returns the op and its byte length.
/// `prev_name` is true when the previous token is a variable name.
pub(crate) fn lex_op(body: &[u8], i: usize, prev_name: bool) -> Option<(Op, usize)> {
    if body.get(i..i + 3) == Some(b"<<=".as_slice()) {
        return Some((Op::ShlAssign, 3));
    }
    if body.get(i..i + 3) == Some(b">>=".as_slice()) {
        return Some((Op::ShrAssign, 3));
    }
    if let Some(op) = inc_dec(body, i, prev_name) {
        return Some(op);
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
        Some(b',') => Some((Op::Comma, 1)),
        _ => None,
    }
}

/// `++`/`--` are an increment/decrement token only when adjacent to a variable
/// name: postfix right after one, prefix right before one. Everywhere else they
/// are two separate `+`/`-` tokens, which is bash's rule: `++1` is `+ (+1)` = 1,
/// `1--1` is `1 - (-1)` = 2, `x++--1` is `x++ - (-1)` = 2.
fn inc_dec(body: &[u8], i: usize, prev_name: bool) -> Option<(Op, usize)> {
    let op = match body.get(i..i + 2) {
        Some(b"++") => Op::Incr,
        Some(b"--") => Op::Decr,
        _ => return None,
    };
    (prev_name || name_ahead(body, i + 2)).then_some((op, 2))
}

/// Is the next non-whitespace byte at `i` the start of a variable name?
fn name_ahead(body: &[u8], mut i: usize) -> bool {
    while let Some(&c) = body.get(i) {
        if matches!(c, b' ' | b'\t' | b'\n' | b'\r') {
            i += 1;
        } else {
            return c.is_ascii_alphabetic() || c == b'_';
        }
    }
    false
}
