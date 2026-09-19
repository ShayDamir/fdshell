//! Number lexing: decimal, `0x…` hex, leading-`0` octal. Overflow wraps mod 2⁶⁴.

use alloc::vec::Vec;
use error_stack::{Report, bail};

use super::lex::Tok;
use crate::error::resolve::ResolveError;

/// Lex the number starting at `i` (a digit); returns the next index.
pub(crate) fn lex_number(
    body: &[u8],
    i: usize,
    toks: &mut Vec<Tok>,
) -> Result<usize, Report<ResolveError>> {
    if is_hex_prefix(body, i) {
        let (value, j) = read_digits(body, i + 2, 16);
        if j == i + 2 {
            bail!(ResolveError::ArithSyntax);
        }
        toks.push(Tok::Num(value));
        return Ok(j);
    }
    if body.get(i) == Some(&b'0') {
        let (value, j) = read_digits(body, i + 1, 8);
        if body
            .get(j)
            .and_then(|&c| digit_value(c, 10))
            .is_some_and(|d| d >= 8)
        {
            // `08` — a digit that is not octal: "value too great for base".
            bail!(ResolveError::ArithSyntax);
        }
        toks.push(Tok::Num(value));
        return Ok(j);
    }
    let (value, j) = read_digits(body, i, 10);
    toks.push(Tok::Num(value));
    Ok(j)
}

fn is_hex_prefix(body: &[u8], i: usize) -> bool {
    body.get(i) == Some(&b'0') && matches!(body.get(i + 1), Some(b'x') | Some(b'X'))
}

/// Consume `radix`-based digits from `i`, wrapping the value mod 2⁶⁴.
fn read_digits(body: &[u8], i: usize, radix: u32) -> (i64, usize) {
    let mut value: i64 = 0;
    let mut j = i;
    while let Some(&c) = body.get(j) {
        let Some(d) = digit_value(c, radix) else {
            break;
        };
        value = value
            .wrapping_mul(i64::from(radix))
            .wrapping_add(i64::from(d));
        j += 1;
    }
    (value, j)
}

fn digit_value(c: u8, radix: u32) -> Option<u32> {
    let value = match c {
        b'0'..=b'9' => u32::from(c - b'0'),
        b'a'..=b'f' => u32::from(c - b'a') + 10,
        b'A'..=b'F' => u32::from(c - b'A') + 10,
        _ => return None,
    };
    (value < radix).then_some(value)
}
