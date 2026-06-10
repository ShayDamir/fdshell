use crate::capture::Capture;
use crate::parse::CommandLine;
use crate::redirect::RedirectDef;
use sys::ShortCStr;
use sys::errno::EINVAL;

pub fn parse_command(tokens: &[ShortCStr]) -> Result<CommandLine, i32> {
    let mut iter = tokens.iter().peekable();
    let builtin = match iter.peek() {
        Some(t) if t.eq_bytes(b"builtin") => {
            iter.next();
            true
        }
        Some(t) => t
            .as_bytes()
            .is_ok_and(|b| matches!(b, b"true" | b"false" | b"pwd")),
        None => false,
    };
    let command = iter.next().ok_or(EINVAL)?.clone();
    let mut args: Vec<ShortCStr> = Vec::new();
    let mut captures: Vec<Capture> = Vec::new();
    let mut redirects: Vec<RedirectDef> = Vec::new();
    let mut pidvar: Option<ShortCStr> = None;
    let mut bg_force = false;
    for t in iter {
        let b = t.as_bytes()?;
        if b == b"&" {
            return Err(EINVAL);
        } else if let Some(rest) = t.strip_prefix(b"&>") {
            let (force, name) = if let Some(name) = rest.strip_prefix(b"|&") {
                (true, name)
            } else if let Some(name) = rest.strip_prefix(b"&") {
                (false, name)
            } else {
                return Err(EINVAL);
            };
            pidvar = Some(name);
            bg_force = force;
        } else if b.starts_with(b"%") {
            if let Some(c) = crate::parse::classify::parse_capture(t) {
                captures.push(c);
            } else {
                args.push(t.clone());
            }
        } else if let Some(r) = crate::parse::classify::parse_redirect(t)? {
            let pos = redirects.binary_search_by_key(&r.export_to, |x| x.export_to);
            match pos {
                Ok(_) => return Err(sys::errno::EEXIST),
                Err(i) => redirects.insert(i, r),
            }
        } else {
            args.push(t.clone());
        }
    }
    Ok(CommandLine {
        builtin,
        command,
        args,
        captures,
        redirects,
        pidvar,
        bg_force,
    })
}
