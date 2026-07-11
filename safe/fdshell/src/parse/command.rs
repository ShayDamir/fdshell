use crate::capture::Capture;
use crate::error::parse::ParseError;
use crate::parse::CommandLine;
use crate::parse::bg_redirect;
use crate::parse::bg_redirect::parse_bg_redirect;
use crate::parse::builtin::is_builtin;
use crate::redirect::RedirectDef;
use error_stack::{Report, bail};
use sys::ShortCStr;

pub fn parse_command(
    tokens: &[ShortCStr],
    fully_quoted: Vec<bool>,
) -> Result<CommandLine, Report<ParseError>> {
    let mut iter = tokens.iter().peekable();
    let builtin = match iter.peek() {
        Some(t) if t.eq_bytes(b"builtin") => {
            iter.next();
            true
        }
        Some(t) => is_builtin(t),
        None => false,
    };
    let command = iter.next().ok_or(ParseError::ExpectedCommand)?.clone();
    let mut args: Vec<ShortCStr> = Vec::new();
    let mut captures: Vec<Capture> = Vec::new();
    let mut redirects: Vec<RedirectDef> = Vec::new();
    let mut pidvar: Option<ShortCStr> = None;
    let mut bg_force = false;
    let mut args_fq = Vec::new();
    let mut fq_iter = fully_quoted.into_iter();
    fq_iter.next();
    if builtin {
        fq_iter.next();
    }
    for t in iter {
        let fq = fq_iter.next().unwrap_or(false);
        if t.as_bytes().is_ok_and(|b| b == b"&") {
            bail!(ParseError::UnexpectedChar { ch: b'&' });
        }
        if let Some(bg) = parse_bg_redirect(t)? {
            if let Some(p) = bg.pidvar {
                pidvar = Some(p);
                bg_force = bg.bg_force;
            } else {
                for r in bg.redirects {
                    bg_redirect::insert_redirect(&mut redirects, r)?;
                }
            }
        } else if t.as_bytes().is_ok_and(|b| b.starts_with(b"%")) {
            match crate::parse::classify::parse_capture(t) {
                Ok(Some(c)) => captures.push(c),
                Ok(None) => {
                    args.push(t.clone());
                    args_fq.push(fq);
                }
                Err(e) => return Err(e),
            }
        } else if let Some(r) = crate::parse::classify::parse_redirect(t)? {
            bg_redirect::insert_redirect(&mut redirects, r)?;
        } else {
            args.push(t.clone());
            args_fq.push(fq);
        }
    }
    Ok(CommandLine {
        builtin,
        command,
        args,
        args_fq,
        captures,
        redirects,
        pidvar,
        bg_force,
    })
}
