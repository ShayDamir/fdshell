mod classify;
mod line;
mod token;

pub use line::ParsedLine;

use crate::capture::Capture;
use crate::redirect::Redirect;
use std::ffi::CString;
use sys::errno::EINVAL;

pub struct CommandLine {
    pub builtin: bool,
    pub command: CString,
    pub args: Vec<CString>,
    pub captures: Vec<Capture>,
    pub redirects: Vec<Redirect>,
    pub background: bool,
}

impl PartialEq for CommandLine {
    fn eq(&self, other: &Self) -> bool {
        self.builtin == other.builtin
            && self.command == other.command
            && self.args == other.args
            && self.captures == other.captures
            && self.redirects == other.redirects
            && self.background == other.background
    }
}

impl core::fmt::Debug for CommandLine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CommandLine")
            .field("builtin", &self.builtin)
            .field("command", &self.command)
            .field("args", &self.args)
            .field("captures", &self.captures)
            .field("redirects", &self.redirects)
            .field("background", &self.background)
            .finish()
    }
}

pub fn parse(line: &str) -> Result<ParsedLine, i32> {
    let raw = token::tokenize(line)?;

    if let Some(pl) = line::detect(&raw)? {
        return Ok(pl);
    }

    let mut iter = raw.iter().peekable();
    let builtin = match iter.peek() {
        Some(t) if t.as_slice() == b"builtin" => {
            iter.next();
            true
        }
        _ => false,
    };
    let command = CString::new(iter.next().ok_or(EINVAL)?.as_slice()).map_err(|_| EINVAL)?;
    let mut args: Vec<CString> = Vec::new();
    let mut captures: Vec<Capture> = Vec::new();
    let mut redirects: Vec<Redirect> = Vec::new();
    let mut background = false;
    for t in iter {
        let b = t.as_slice();
        if b == b"&" {
            background = true;
        } else if b.starts_with(b"%") {
            if let Some(c) = classify::parse_capture(b) {
                captures.push(c);
            } else {
                args.push(CString::new(b).map_err(|_| EINVAL)?);
            }
        } else if let Some(r) = classify::parse_redirect(b) {
            redirects.push(r);
        } else {
            args.push(CString::new(b).map_err(|_| EINVAL)?);
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
