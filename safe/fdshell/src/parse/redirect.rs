use crate::error::parse::ParseError;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use error_stack::{Report, ResultExt, ensure};
use sys::ShortCStr;

fn parse_fd(prefix: &[u8], dir: u8) -> Option<i32> {
    if prefix.is_empty() {
        Some(match dir {
            b'<' => 0,
            _ => 1,
        })
    } else if prefix.iter().all(|c| c.is_ascii_digit()) {
        core::str::from_utf8(prefix).ok()?.parse().ok()
    } else {
        None
    }
}

fn parse_path_redirect(
    after_op: ShortCStr,
    prefix: &[u8],
    dir: u8,
) -> Result<Option<RedirectDef>, Report<ParseError>> {
    let (rest, direction) = if dir == b'>' && after_op.starts_with(b">") {
        let r = after_op.get(1..).ok_or(ParseError::InvalidRedirect)?;
        ensure!(
            !(r.is_empty() || r.starts_with(b"%")),
            ParseError::InvalidRedirect
        );
        (r, RedirectDirection::Append)
    } else if dir == b'<' {
        (after_op, RedirectDirection::Read)
    } else {
        (after_op, RedirectDirection::Write)
    };
    if let Some(export_to) = parse_fd(prefix, dir) {
        Ok(Some(RedirectDef {
            export_to,
            direction,
            source: RedirectSource::path(rest),
        }))
    } else {
        Ok(None)
    }
}

pub fn parse_redirect(s: &ShortCStr) -> Result<Option<RedirectDef>, Report<ParseError>> {
    let bytes = s.as_bytes().change_context(ParseError::Never)?;
    let op_pos = match bytes.iter().position(|&b| b == b'>' || b == b'<') {
        Some(p) => p,
        None => return Ok(None),
    };
    let dir = match bytes.get(op_pos) {
        Some(&d) => d,
        None => return Ok(None),
    };
    let after_op = match s.get(op_pos + 1..) {
        Some(r) => r,
        None => return Ok(None),
    };
    if after_op.is_empty() || after_op.starts_with(b"&") {
        return Ok(None);
    }
    let prefix = match bytes.get(..op_pos) {
        Some(p) => p,
        None => return Ok(None),
    };
    if after_op.starts_with(b"%") {
        let source = after_op.get(1..).ok_or(ParseError::InvalidRedirect)?;
        if let Some(export_to) = parse_fd(prefix, dir) {
            Ok(Some(RedirectDef::var(export_to, source)))
        } else {
            Ok(None)
        }
    } else {
        parse_path_redirect(after_op, prefix, dir)
    }
}
