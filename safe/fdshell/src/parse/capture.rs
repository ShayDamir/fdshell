use crate::capture::Capture;
use crate::error::parse::ParseError;
use error_stack::{Report, bail};
use sys::Position;
use sys::ShortCStr;

pub fn parse_capture(
    s: &ShortCStr,
    set_at: Position,
) -> Result<Option<Capture>, Report<ParseError>> {
    let s = match s.strip_prefix(b"%") {
        Some(s) => s,
        None => return Ok(None),
    };
    let (tag_part, mut rest) = match s.split_once_byte(b'>') {
        Some(pair) => pair,
        None => return Ok(None),
    };
    let force = rest.strip_prefix(b"|").is_some();
    if force {
        rest = rest.get(1..).ok_or(ParseError::CaptureEmptyVar)?;
    }
    let var_name = match rest.strip_prefix(b"%") {
        Some(v) if v.is_empty() => bail!(ParseError::CaptureEmptyVar),
        Some(v) => v,
        None => bail!(ParseError::CaptureMissingPercent),
    };
    let (var, cap) = match crate::parse::array_ref::split_index_ref(&var_name) {
        Some((base, idx)) => (base, Some(idx)),
        None => (var_name, None),
    };
    Ok(Some(Capture {
        var,
        tag: if tag_part.is_empty() {
            None
        } else {
            Some(tag_part)
        },
        force,
        cap,
        set_at,
    }))
}
