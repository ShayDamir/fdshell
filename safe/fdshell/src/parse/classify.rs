use crate::capture::Capture;
use crate::redirect::Redirect;
use sys::ShortCStr;

pub(crate) fn parse_capture(bytes: &[u8]) -> Option<Capture> {
    let pos = bytes.iter().position(|&c| c == b'>')?;
    let tag_part = bytes.get(1..pos)?;
    let mut rest = bytes.get(pos + 1..)?;
    let force = rest.first() == Some(&b'|');
    if force {
        rest = rest.get(1..)?;
    }
    if rest.first() != Some(&b'%') {
        return None;
    }
    let var_name = rest.get(1..)?;
    if var_name.is_empty() {
        return None;
    }
    Some(Capture {
        var: ShortCStr::from_bytes(var_name).ok()?,
        tag: {
            if tag_part.is_empty() {
                None
            } else {
                Some(ShortCStr::from_bytes(tag_part).ok()?)
            }
        },
        force,
    })
}

pub(crate) fn parse_redirect(bytes: &[u8]) -> Option<Redirect> {
    let (pos, dir) = if let Some(p) = bytes.windows(2).position(|w| w == b">%") {
        (p, b'>')
    } else if let Some(p) = bytes.windows(2).position(|w| w == b"<%") {
        (p, b'<')
    } else {
        return None;
    };
    let prefix = bytes.get(..pos)?;
    let var_name = bytes.get(pos + 2..)?;
    if var_name.is_empty() {
        return None;
    }
    let target_fd = if prefix.is_empty() {
        match dir {
            b'<' => 0,
            _ => 1,
        }
    } else if prefix.iter().all(|c| c.is_ascii_digit()) {
        let s = core::str::from_utf8(prefix).ok()?;
        s.parse().ok()?
    } else {
        return None;
    };
    Some(Redirect {
        target_fd,
        src_var: ShortCStr::from_bytes(var_name).ok()?,
    })
}
