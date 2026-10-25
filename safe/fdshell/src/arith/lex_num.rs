//! Number lexing: decimal, `base#digits`, `0x…` hex, leading-`0` octal.
//! Overflow wraps mod 2⁶⁴.

use alloc::vec::Vec;
use error_stack::{Report, bail, ensure};

use super::lex::Tok;
use crate::error::resolve::ResolveError;

/// Lex the number starting at `i` (a digit); returns the next index.
pub(crate) fn lex_number(
    body: &[u8],
    i: usize,
    toks: &mut Vec<Tok>,
) -> Result<usize, Report<ResolveError>> {
    let (value, j) = read_digits(body, i, 10);
    if body.get(j) == Some(&b'#') {
        return lex_radix(body, i, j, value, toks);
    }
    if is_hex_prefix(body, i) {
        let (value, j) = read_digits(body, i + 2, 16);
        if j == i + 2 {
            bail!(ResolveError::ArithSyntax);
        }
        toks.push(Tok::Num(value));
        return Ok(j);
    }
    if body.get(i) == Some(&b'0') {
        // Octal: the scan is seeded at the leading `0` — it contributes value 0
        // in base 8 and advances the run by exactly one, so no `+ 1` offset is
        // needed (an offset there is a mutant source, not a rule).
        let (value, j) = read_digits(body, i, 8);
        toks.push(Tok::Num(value));
        return Ok(j);
    }
    // Decimal: reuse the pre-read run.
    toks.push(Tok::Num(value));
    Ok(j)
}

/// Lex `base#digits` whose base digits span `body[i..h)` (the `#` is at `h`).
fn lex_radix(
    body: &[u8],
    i: usize,
    h: usize,
    base: i64,
    toks: &mut Vec<Tok>,
) -> Result<usize, Report<ResolveError>> {
    // bash: a leading-zero base (`08#1`) is not a number.
    ensure!(
        h == i + 1 || body.get(i) != Some(&b'0'),
        ResolveError::ArithSyntax
    );
    let Some(radix) = (2..=64).contains(&base).then_some(base as u32) else {
        bail!(ResolveError::ArithSyntax);
    };
    let (value, j) = read_digits(body, h + 1, radix);
    ensure!(j > h + 1, ResolveError::ArithSyntax);
    // A digit outside the base (`2#102`) ends the run, so it lexes as its own
    // number token and the parser's trailing-input bail rejects the pair —
    // the same `ArithSyntax` a local "value too great for base" guard
    // reported, from the rule that already owns the check.
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
        b'a'..=b'z' => u32::from(c - b'a') + 10,
        b'A'..=b'Z' => u32::from(c - b'A') + 10,
        _ => return None,
    };
    (value < radix).then_some(value)
}
