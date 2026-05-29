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

pub fn parse_redirect(s: &ShortCStr) -> Option<RedirectDef> {
    let bytes = s.as_bytes();

    if let Some(pos) = bytes.windows(2).position(|w| w == b">%") {
        return Some(RedirectDef {
            export_to: parse_fd(bytes.get(..pos)?, b'>')?,
            direction: RedirectDirection::Write,
            source: RedirectSource::Var(s.get(pos + 2..)?),
        });
    }
    if let Some(pos) = bytes.windows(2).position(|w| w == b"<%") {
        return Some(RedirectDef {
            export_to: parse_fd(bytes.get(..pos)?, b'<')?,
            direction: RedirectDirection::Read,
            source: RedirectSource::Var(s.get(pos + 2..)?),
        });
    }

    let op_pos = bytes.iter().position(|&b| b == b'>' || b == b'<')?;
    let dir = *bytes.get(op_pos)?;
    let rest = s.get(op_pos + 1..)?;
    if rest.is_empty() || rest.as_bytes().starts_with(b"&") {
        return None;
    }
    Some(RedirectDef {
        export_to: parse_fd(bytes.get(..op_pos)?, dir)?,
        direction: if dir == b'<' {
            RedirectDirection::Read
        } else {
            RedirectDirection::Write
        },
        source: RedirectSource::Path(rest),
    })
}
