use crate::capture::Capture;
use crate::parse::{CommandLine, ParsedLine, Pipeline};
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
        _ => false,
    };
    let command = iter.next().ok_or(EINVAL)?.clone();
    let mut args: Vec<ShortCStr> = Vec::new();
    let mut captures: Vec<Capture> = Vec::new();
    let mut redirects: Vec<RedirectDef> = Vec::new();
    let mut background = false;
    for t in iter {
        let b = t.as_bytes()?;
        if b == b"&" {
            background = true;
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
        background,
    })
}

pub fn parse_pipeline(raw: &[ShortCStr]) -> Result<ParsedLine, i32> {
    let mut commands = Vec::new();
    let mut start = 0;
    for (i, t) in raw.iter().enumerate() {
        if t.as_bytes()? == b"|" {
            if i == start {
                return Err(EINVAL);
            }
            commands.push(parse_command(raw.get(start..i).ok_or(EINVAL)?)?);
            start = i + 1;
        }
    }
    if start >= raw.len() {
        return Err(EINVAL);
    }
    commands.push(parse_command(raw.get(start..).ok_or(EINVAL)?)?);
    Ok(ParsedLine::Pipeline(Pipeline { commands }))
}
