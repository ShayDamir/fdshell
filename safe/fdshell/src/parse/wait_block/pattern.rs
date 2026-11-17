// mutants: -C --lib -C --test -C wait -- -E 'test(/^parse::wait_block::/) | binary(wait)'

use super::fdref::parse_fdref;
use super::{FdRef, WaitPattern};
use crate::capture::Capture;
use crate::error::parse::{ParseError, WaitError};
use crate::parse::Token;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail, ensure};
use sys::ShortCStr;

pub(super) fn parse_pattern(
    tokens: &[Token],
    start: sys::Position,
) -> Result<(WaitPattern, Vec<Capture>), Report<ParseError>> {
    let (kw, rest) = match tokens.first() {
        Some(Token { word: kw, .. }) => (kw, rest_from(tokens, 1)?),
        None => bail!(ParseError::Wait(WaitError::EmptyPattern)),
    };
    if let Some(make) = wait_kind(kw) {
        arm(make, rest, start)
    } else if kw.eq_bytes(b"after") {
        after(rest)
    } else {
        bail!(ParseError::Wait(WaitError::UnknownPattern))
    }
}

fn rest_from(tokens: &[Token], from: usize) -> Result<&[Token], Report<ParseError>> {
    tokens.get(from..).ok_or(ParseError::Never.into())
}

fn wait_kind(kw: &ShortCStr) -> Option<fn(FdRef) -> WaitPattern> {
    if kw.eq_bytes(b"readable") {
        Some(WaitPattern::Readable)
    } else if kw.eq_bytes(b"writable") {
        Some(WaitPattern::Writable)
    } else if kw.eq_bytes(b"finished") {
        Some(WaitPattern::Finished)
    } else {
        None
    }
}

fn arm(
    make: fn(FdRef) -> WaitPattern,
    rest: &[Token],
    start: sys::Position,
) -> Result<(WaitPattern, Vec<Capture>), Report<ParseError>> {
    let (fd_tok, caps) = match rest.first() {
        Some(Token { word: fd, .. }) => (fd, rest_from(rest, 1)?),
        None => bail!(ParseError::Wait(WaitError::MissingFd)),
    };
    let ref_ = parse_fdref(fd_tok)?;
    let captures = parse_captures(caps, start)?;
    Ok((make(ref_), captures))
}

fn after(rest: &[Token]) -> Result<(WaitPattern, Vec<Capture>), Report<ParseError>> {
    let ms_tok = rest
        .first()
        .map(|t| &t.word)
        .ok_or(ParseError::Wait(WaitError::MissingTimeout))?;
    let ms = ms_tok
        .parse::<usize>()
        .change_context(ParseError::Wait(WaitError::InvalidTimeout))?;
    ensure!(
        rest.len() <= 1,
        ParseError::Wait(WaitError::UnexpectedToken)
    );
    Ok((WaitPattern::After(ms), Vec::new()))
}

fn parse_captures(
    toks: &[Token],
    start: sys::Position,
) -> Result<Vec<Capture>, Report<ParseError>> {
    let mut out = Vec::new();
    for Token { word: t, .. } in toks {
        match crate::parse::capture::parse_capture(t, start)? {
            Some(c) => out.push(c),
            None => bail!(ParseError::Wait(WaitError::UnexpectedToken)),
        }
    }
    Ok(out)
}
