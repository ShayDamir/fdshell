use crate::capture::Capture;
use crate::redirect::Redirect;
use sys::ShortCStr;

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

pub fn parse_redirect(s: &ShortCStr) -> Option<Redirect> {
    let bytes = s.as_bytes();
    let (pos, dir) = if let Some(p) = bytes.windows(2).position(|w| w == b">%") {
        (p, b'>')
    } else if let Some(p) = bytes.windows(2).position(|w| w == b"<%") {
        (p, b'<')
    } else {
        return None;
    };
    let prefix = bytes.get(..pos)?;
    let var_name = s.get(pos + 2..)?;
    let target_fd = if prefix.is_empty() {
        match dir {
            b'<' => 0,
            _ => 1,
        }
    } else if prefix.iter().all(|c| c.is_ascii_digit()) {
        let digits = core::str::from_utf8(prefix).ok()?;
        digits.parse().ok()?
    } else {
        return None;
    };
    Some(Redirect {
        target_fd,
        src_var: var_name,
    })
}
