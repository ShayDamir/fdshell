//! The body-slice rule of `parse/semi.rs`: which bracketing `;` a body drops
//! and which it **keeps**. The plain `&` terminator stays in the body so the
//! body's own scan makes the background job (task #354), and the `;` glued
//! right behind that `&` stays too, so the body's own scan rejects the split
//! (task #355, bash's rc-2 family).

use super::Token;
use super::preceded::amp_semi;
use crate::error::parse::ParseError;
use error_stack::{Report, ensure};

/// The exclusive end of a body slice written **before** the closer at
/// `closer_idx`: POSIX's `;` there is dropped (the body text ends at its last
/// command), a plain-`&` terminator is **kept** — the body's own scan turns it
/// into a background job (task #354) — and so is a `;` glued right behind that
/// `&`: the body text carries `& ;` and the body's own scan rejects the split
/// (task #355). Any other word the closer is glued to stays in the body, as
/// `if_block` applies today.
pub(crate) fn body_end(tokens: &[Token], line: &[u8], closer_idx: usize) -> usize {
    let before = closer_idx.saturating_sub(1);
    let glued = tokens
        .get(before)
        .is_some_and(|(t, _, _, _, _, _)| !t.eq_bytes(b";"));
    if glued || amp_semi(tokens, line, before) {
        closer_idx
    } else {
        before
    }
}

/// A **statement-body** slice: the `;`s bracketing it trimmed, except a `;`
/// glued right behind a plain `&` ([`amp_semi`], task #355). Bash reads
/// `echo a & ;` as a syntax error that runs nothing; the kept `;` makes the
/// body text carry `& ;`, and the body's own scan is what says so. The
/// newline/`;`-byte check lives in `amp_semi`: the tokenizer emits a `;`
/// token for a newline too, and a body ending `&` newline is the
/// bash-accepted `&`-terminated body (`for i in 1; do echo a &\ndone`).
pub(crate) fn trim_body<'a>(tokens: &'a [Token], line: &[u8]) -> &'a [Token] {
    let start = tokens
        .iter()
        .take_while(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
        .count();
    let mut end = tokens.len();
    while end > 0
        && tokens
            .get(end - 1)
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
    {
        if amp_semi(tokens, line, end - 1) {
            break;
        }
        end -= 1;
    }
    tokens.get(start..end).unwrap_or(&[])
}

/// The **non-statement** slices keep the plain rule: a `for` word list, a
/// `case` pattern, the `case` word, a `wait` arm's patterns — the `& ;` glue
/// is statement text, and a word list is not (task #355; `for i in 1 & ; do`
/// pins the divergence).
pub(crate) fn trim_semi(tokens: &[Token]) -> &[Token] {
    let start = tokens
        .iter()
        .take_while(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
        .count();
    let end = tokens
        .iter()
        .rev()
        .take_while(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
        .count();
    let end = tokens.len().saturating_sub(end);
    tokens.get(start..end).unwrap_or(&[])
}

/// A **condition** slice: on top of [`trim_body`], the plain `&` that precedes
/// a clause keyword terminates the compound list, and the condition is run as
/// its own command list (its scanner never saw the `&`, so nothing here
/// consumes it): `if true & then …` runs `true` as the condition (task #354;
/// the background job of a `&`-terminated condition is #356's). A `& ;`
/// behind the condition keeps its `;` (task #355), so the condition text
/// carries the split its own scan rejects.
pub(crate) fn trim_condition<'a>(tokens: &'a [Token], line: &[u8]) -> &'a [Token] {
    let body = trim_body(tokens, line);
    let amp = usize::from(
        body.last()
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"&")),
    );
    let end = body.len().saturating_sub(amp);
    body.get(..end).unwrap_or(&[])
}

/// A block arm's body tokens: `tokens` with the `;`s bracketing it trimmed
/// ([`trim_body`]), `err` when nothing is left — bash rejects an empty body
/// (`if x; then; fi`, `while x; do; done`, `for i in a; do; done`: all rc 2,
/// and an empty `do` body makes the loop unbounded at run time).
pub(crate) fn block_body<'a>(
    tokens: &'a [Token],
    line: &[u8],
    err: ParseError,
) -> Result<&'a [Token], Report<ParseError>> {
    let body = trim_body(tokens, line);
    ensure!(!body.is_empty(), err);
    Ok(body)
}

#[cfg(test)]
mod tests;
