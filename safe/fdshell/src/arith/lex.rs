//! Lexer for `$((…))` bodies: whitespace, numbers, names, `$$`/`$!`, `$name`,
//! parens and operators.

use alloc::vec::Vec;
use error_stack::{Report, bail};
use sys::ShortCStr;

use super::lex_name::{lex_dollar, lex_name};
use super::lex_num::lex_number;
use super::lex_op::lex_op;
use super::op::Op;
use crate::error::resolve::ResolveError;

/// One token of an arithmetic expression.
#[derive(Clone)]
pub(crate) enum Tok {
    Num(i64),
    Name(ShortCStr),
    Pid,
    LastBg,
    Op(Op),
    LParen,
    RParen,
}

pub(crate) fn lex(body: &[u8]) -> Result<Vec<Tok>, Report<ResolveError>> {
    let mut toks = Vec::new();
    let mut i = 0usize;
    while let Some(&c) = body.get(i) {
        i = lex_one(c, body, i, &mut toks)?;
    }
    Ok(toks)
}

/// Lex one token starting at `i` (where `body.get(i) == c`); returns the next index.
fn lex_one(
    c: u8,
    body: &[u8],
    i: usize,
    toks: &mut Vec<Tok>,
) -> Result<usize, Report<ResolveError>> {
    if matches!(c, b' ' | b'\t' | b'\n' | b'\r') {
        return Ok(i + 1);
    }
    if c.is_ascii_digit() {
        return lex_number(body, i, toks);
    }
    if c.is_ascii_alphabetic() || c == b'_' {
        return lex_name(body, i, toks);
    }
    if c == b'$' {
        return lex_dollar(body, i, toks);
    }
    if c == b'(' {
        toks.push(Tok::LParen);
        return Ok(i + 1);
    }
    if c == b')' {
        toks.push(Tok::RParen);
        return Ok(i + 1);
    }
    match lex_op(body, i) {
        Some((op, len)) => {
            toks.push(Tok::Op(op));
            Ok(i + len)
        }
        None => bail!(ResolveError::ArithSyntax),
    }
}
