use crate::capture::Capture;
use crate::error::parse::ParseError;
use crate::parse::{CommandLine, bg_redirect, bg_redirect::parse_bg_redirect};
use crate::redirect::RedirectDef;
use alloc::vec::{IntoIter, Vec};
use error_stack::{Report, bail};
use sys::{Position, ShortCStr};

/// Collect the tokens after the command word into args, captures and
/// redirects, returning the finished `CommandLine`.
pub(super) fn finish_command(
    builtin: bool,
    command: ShortCStr,
    iter: &mut core::iter::Peekable<core::slice::Iter<'_, ShortCStr>>,
    fq_iter: &mut IntoIter<bool>,
    set_at: Position,
) -> Result<CommandLine, Report<ParseError>> {
    let mut args: Vec<ShortCStr> = Vec::new();
    let mut captures: Vec<Capture> = Vec::new();
    let mut redirects: Vec<RedirectDef> = Vec::new();
    let mut pidvar: Option<ShortCStr> = None;
    let mut bg_force = false;
    let mut args_fq = Vec::new();
    while let Some(t) = iter.next() {
        let fq = fq_iter.next().unwrap_or(false);
        if t.eq_bytes(b"&") {
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
        } else if t.starts_with(b"%") {
            match crate::parse::classify::parse_capture(t, set_at) {
                Ok(Some(c)) => captures.push(c),
                Ok(None) => {
                    args.push(t.clone());
                    args_fq.push(fq);
                }
                Err(e) => return Err(e),
            }
        } else if let Some(r) = crate::parse::classify::parse_here_string(t, fq, iter, fq_iter)? {
            bg_redirect::insert_redirect(&mut redirects, r)?;
        } else if let Some(r) = crate::parse::classify::parse_redirect(t, fq)? {
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
