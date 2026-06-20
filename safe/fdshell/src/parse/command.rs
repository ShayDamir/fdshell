use crate::capture::Capture;
use crate::error::parse::ParseError;
use crate::error::parse::{report_error, report_invalid_char};
use crate::parse::CommandLine;
use crate::redirect::RedirectDef;
use error_stack::Report;
use sys::ShortCStr;

pub fn parse_command(tokens: &[ShortCStr]) -> Result<CommandLine, Report<ParseError>> {
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
    let command = iter
        .next()
        .ok_or_else(|| report_error("expected command", 0))?
        .clone();
    let mut args: Vec<ShortCStr> = Vec::new();
    let mut captures: Vec<Capture> = Vec::new();
    let mut redirects: Vec<RedirectDef> = Vec::new();
    let mut pidvar: Option<ShortCStr> = None;
    let mut bg_force = false;
    for t in iter {
        let b = t.as_bytes().map_err(|_| report_invalid_char(0, 0))?;
        if b == b"&" {
            return Err(report_error("unexpected '&'", 0));
        } else if let Some(rest) = t.strip_prefix(b"&>") {
            let (force, name) = if let Some(name) = rest.strip_prefix(b"|&") {
                (true, name)
            } else if let Some(name) = rest.strip_prefix(b"&") {
                (false, name)
            } else {
                return Err(report_error("invalid '&>' syntax", 0));
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
                Ok(_) => return Err(report_error("duplicate redirect", 0)),
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
