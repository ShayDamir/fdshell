use crate::capture::Capture;
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
