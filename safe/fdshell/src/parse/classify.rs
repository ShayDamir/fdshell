use crate::capture::Capture;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
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

pub fn parse_capture(s: &ShortCStr) -> Option<Capture> {
    let s = s.strip_prefix(b"%")?;
    let (tag_part, mut rest) = s.split_once_byte(b'>')?;
    let force = rest.strip_prefix(b"|").is_some();
    if force {
        rest = rest.get(1..)?;
    }
    let var_name = rest.strip_prefix(b"%")?;
    Some(Capture {
        var: var_name,
        tag: if tag_part.is_empty() {
            None
        } else {
            Some(tag_part)
        },
        force,
    })
}

pub fn parse_redirect(s: &ShortCStr) -> Result<Option<RedirectDef>, i32> {
    let bytes = s.as_bytes()?;

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

    let prefix = bytes.get(..op_pos).ok_or(sys::errno::EINVAL)?;

    if after_op.starts_with(b"%") {
        let source = after_op.get(1..).ok_or(sys::errno::EINVAL)?;
        let export_to = match parse_fd(prefix, dir) {
            Some(fd) => fd,
            None => return Ok(None),
        };
        return Ok(Some(RedirectDef::var(export_to, source)));
    }

    let (rest, direction) = if dir == b'>' && after_op.starts_with(b">") {
        let r = after_op.get(1..).ok_or(sys::errno::EINVAL)?;
        if r.is_empty() || r.starts_with(b"%") {
            return Err(sys::errno::EINVAL);
        }
        (r, RedirectDirection::Append)
    } else if dir == b'<' {
        (after_op, RedirectDirection::Read)
    } else {
        (after_op, RedirectDirection::Write)
    };

    let export_to = match parse_fd(prefix, dir) {
        Some(fd) => fd,
        None => return Ok(None),
    };
    Ok(Some(RedirectDef {
        export_to,
        direction,
        source: RedirectSource::path(rest),
    }))
}
