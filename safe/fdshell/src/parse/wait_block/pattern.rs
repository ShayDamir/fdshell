use super::{FdRef, WaitPattern};
use crate::capture::Capture;
use crate::error::parse::ParseError;
use crate::parse::Token;
use alloc::vec::Vec;
use error_stack::{Report, bail, ensure};
use sys::ShortCStr;

pub(super) fn parse_pattern(
    tokens: &[Token],
    start: sys::Position,
) -> Result<(WaitPattern, Vec<Capture>), Report<ParseError>> {
    let (kw, rest) = match tokens.first() {
        Some((kw, _, _, _)) => (kw, rest_from(tokens, 1)?),
        None => bail!(ParseError::WaitEmptyPattern),
    };
    if let Some(make) = wait_kind(kw) {
        arm(make, rest, start)
    } else if kw.eq_bytes(b"after") {
        after(rest)
    } else {
        bail!(ParseError::WaitUnknownPattern)
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
        Some((fd, _, _, _)) => (fd, rest_from(rest, 1)?),
        None => bail!(ParseError::WaitMissingFd),
    };
    let ref_ = parse_fdref(fd_tok)?;
    let captures = parse_captures(caps, start)?;
    Ok((make(ref_), captures))
}

fn after(rest: &[Token]) -> Result<(WaitPattern, Vec<Capture>), Report<ParseError>> {
    let ms_tok = rest
        .first()
        .map(|t| &t.0)
        .ok_or(ParseError::WaitMissingTimeout)?;
    let ms = ms_tok
        .parse::<usize>()
        .map_err(|_| Report::new(ParseError::WaitInvalidTimeout))?;
    ensure!(rest.len() <= 1, ParseError::WaitUnexpectedToken);
    Ok((WaitPattern::After(ms), Vec::new()))
}

fn parse_fdref(tok: &ShortCStr) -> Result<FdRef, Report<ParseError>> {
    let rest = tok.strip_prefix(b"%").ok_or(ParseError::WaitFdRefPercent)?;
    if let Some(task) = rest.strip_prefix(b"&") {
        ensure!(!task.is_empty(), ParseError::WaitMissingFd);
        return Ok(FdRef::Task(task));
    }
    if rest.ends_with(b"[]") {
        let base = rest
            .get(..rest.len() - 2)
            .ok_or(ParseError::WaitMissingFd)?;
        ensure!(!base.is_empty(), ParseError::WaitMissingFd);
        return Ok(FdRef::Array(base));
    }
    ensure!(!rest.is_empty(), ParseError::WaitMissingFd);
    Ok(FdRef::Var(rest))
}

fn parse_captures(
    toks: &[Token],
    start: sys::Position,
) -> Result<Vec<Capture>, Report<ParseError>> {
    let mut out = Vec::new();
    for (t, _, _, _) in toks {
        match crate::parse::capture::parse_capture(t, start)? {
            Some(c) => out.push(c),
            None => bail!(ParseError::WaitUnexpectedToken),
        }
    }
    Ok(out)
}
