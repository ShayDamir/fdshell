mod classify;
mod cmdline;
mod line;
mod token;

pub use cmdline::CommandLine;
pub use line::ParsedLine;

use crate::capture::Capture;
use crate::redirect::Redirect;
use sys::ShortCStr;
use sys::errno::EINVAL;

pub fn parse(line: &str) -> Result<ParsedLine, i32> {
    let raw = token::tokenize(line)?;

    if let Some(pl) = line::detect(&raw)? {
        return Ok(pl);
    }

    let mut iter = raw.iter().peekable();
    let builtin = match iter.peek() {
        Some(t) if t.as_bytes() == b"builtin" => {
            iter.next();
            true
        }
        _ => false,
    };
    let command = iter.next().ok_or(EINVAL)?.clone();
    let mut args: Vec<ShortCStr> = Vec::new();
    let mut captures: Vec<Capture> = Vec::new();
    let mut redirects: Vec<Redirect> = Vec::new();
    let mut background = false;
    for t in iter {
        let b = t.as_bytes();
        if b == b"&" {
            background = true;
        } else if b.starts_with(b"%") {
            if let Some(c) = classify::parse_capture(t) {
                captures.push(c);
            } else {
                args.push(t.clone());
            }
        } else if let Some(r) = classify::parse_redirect(t) {
            redirects.push(r);
        } else {
            args.push(t.clone());
        }
    }
    Ok(ParsedLine::Cmd(CommandLine {
        builtin,
        command,
        args,
        captures,
        redirects,
        background,
    }))
}

#[cfg(test)]
mod tests;
