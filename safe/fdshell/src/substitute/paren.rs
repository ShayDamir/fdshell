use alloc::vec::Vec;
use error_stack::{Report, bail};

use crate::error::resolve::ResolveError;

pub fn read_paren_expr(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
) -> Result<Vec<u8>, Report<ResolveError>> {
    let mut inner = Vec::new();
    let mut depth = 1u32;
    let mut in_quotes = false;
    loop {
        let Some(c) = peek.next() else {
            bail!(ResolveError::UnclosedParen);
        };
        // Same quoting rules as the parse-time scanner: quoted parens are
        // data, and a backslash inside quotes shields the next byte.
        if in_quotes && c == b'\\' {
            let Some(escaped) = peek.next() else {
                bail!(ResolveError::UnclosedParen);
            };
            inner.push(b'\\');
            inner.push(escaped);
            continue;
        }
        if c == b')' && !in_quotes && depth == 1 {
            return Ok(inner);
        }
        inner.push(c);
        match c {
            b'"' => in_quotes = !in_quotes,
            b'(' if !in_quotes => depth += 1,
            b')' if !in_quotes => depth -= 1,
            _ => {}
        }
    }
}
