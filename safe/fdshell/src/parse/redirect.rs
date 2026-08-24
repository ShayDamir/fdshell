use crate::error::parse::ParseError;
use crate::redirect::{RedirectDef, RedirectSource};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(super) fn parse_fd(prefix: &ShortCStr, dir: u8) -> Option<i32> {
    if prefix.is_empty() {
        Some(match dir {
            b'<' => 0,
            _ => 1,
        })
    } else {
        prefix.parse().ok()
    }
}

fn parse_path_redirect(
    after_op: ShortCStr,
    prefix: ShortCStr,
    dir: u8,
) -> Result<Option<RedirectDef>, Report<ParseError>> {
    let (rest, direction) = super::redirect_op::redirect_op(dir, after_op)?;
    let Some(export_to) = parse_fd(&prefix, dir) else {
        return Ok(None);
    };
    if let Some(n) = super::fd_path::fd_path_target(&rest) {
        return Ok(Some(RedirectDef::dup(export_to, n)));
    }
    Ok(Some(RedirectDef {
        export_to,
        direction,
        source: RedirectSource::path(rest),
    }))
}

pub fn parse_redirect(s: &ShortCStr, fq: bool) -> Result<Option<RedirectDef>, Report<ParseError>> {
    if fq {
        return Ok(None);
    }
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
    if after_op.is_empty() {
        return Ok(None);
    }
    let prefix = match s.get(..op_pos) {
        Some(p) => p,
        None => return Ok(None),
    };
    if after_op.starts_with(b"&") {
        return super::fd_dup::parse_fd_dup_redirect(&after_op, &prefix, dir);
    }
    if after_op.starts_with(b"%") {
        let source = after_op.get(1..).ok_or(ParseError::InvalidRedirect)?;
        if let Some(export_to) = parse_fd(&prefix, dir) {
            Ok(Some(RedirectDef::var(export_to, source)))
        } else {
            Ok(None)
        }
    } else {
        parse_path_redirect(after_op, prefix, dir)
    }
}
