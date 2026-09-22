use crate::capture::Capture;
use crate::error::parse::ParseError;
use crate::parse::bg_redirect::parse_bg_redirect;
use crate::parse::{CommandLine, Token, bg_redirect};
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use error_stack::{Report, bail};
use sys::{Position, ShortCStr};

/// Collect the tokens after the command word into args, captures and
/// redirects, returning the finished `CommandLine`. Tokens after the first
/// `;` are the heredoc body region: they belong to the body, not the command.
pub(super) fn finish_command(
    builtin: bool,
    command: ShortCStr,
    tokens: &[Token],
    args_from: usize,
    line: &[u8],
    specs: &[crate::parse::heredoc::HeredocBody],
    set_at: Position,
) -> Result<CommandLine, Report<ParseError>> {
    let mut args: Vec<ShortCStr> = Vec::new();
    let mut captures: Vec<Capture> = Vec::new();
    let mut redirects: Vec<RedirectDef> = Vec::new();
    let mut pidvar: Option<ShortCStr> = None;
    let mut bg_force = false;
    let mut args_mask = Vec::new();
    let mut spec_at = 0usize;
    let mut i = args_from;
    while let Some((t, _ts, _te, fq, mask)) = tokens.get(i) {
        if t.eq_bytes(b";") {
            break;
        }
        if t.eq_bytes(b"&") {
            bail!(ParseError::UnexpectedChar { ch: b'&' });
        }
        let mut skip = 0usize;
        if super::heredoc::is_operator(tokens, i) {
            let spec = specs.get(spec_at).ok_or(ParseError::InvalidRedirect)?;
            let (r, extra) = super::heredoc::parse_operator(line, tokens, i, spec)?;
            bg_redirect::insert_redirect(&mut redirects, r)?;
            spec_at += 1;
            skip = extra;
        } else if let Some(bg) = parse_bg_redirect(t)? {
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
                    args_mask.push(mask.clone());
                }
                Err(e) => return Err(e),
            }
        } else if let Some((r, extra)) = super::here_string::parse_here_string(tokens, i)? {
            bg_redirect::insert_redirect(&mut redirects, r)?;
            skip = extra;
        } else if let Some(r) = crate::parse::classify::parse_redirect(t, *fq)? {
            bg_redirect::insert_redirect(&mut redirects, r)?;
        } else {
            args.push(t.clone());
            args_mask.push(mask.clone());
        }
        i += 1 + skip;
    }
    Ok(CommandLine {
        builtin,
        command,
        args,
        args_mask,
        captures,
        redirects,
        pidvar,
        bg_force,
    })
}
