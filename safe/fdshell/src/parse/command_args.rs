use crate::error::parse::ParseError;
use crate::parse::bg_redirect::parse_bg_redirect;
use crate::parse::{CommandLine, Token, bg_redirect};
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
    let mut cmd = CommandLine {
        builtin,
        command,
        args: Vec::new(),
        args_mask: Vec::new(),
        args_quoted: Vec::new(),
        captures: Vec::new(),
        redirects: Vec::new(),
        pidvar: None,
        bg_force: false,
    };
    let mut spec_at = 0usize;
    let mut i = args_from;
    while let Some((t, ts, te, fq, mask)) = tokens.get(i) {
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
            bg_redirect::insert_redirect(&mut cmd.redirects, r)?;
            spec_at += 1;
            skip = extra;
        } else if let Some(bg) = parse_bg_redirect(t)? {
            // A pidvar bg redirect carries no redirects, so the loop is a
            // no-op there; the two cases share one straight-line form.
            for r in bg.redirects {
                bg_redirect::insert_redirect(&mut cmd.redirects, r)?;
            }
            if let Some(p) = bg.pidvar {
                cmd.pidvar = Some(p);
                cmd.bg_force = bg.bg_force;
            }
        } else if t.starts_with(b"%") {
            match crate::parse::classify::parse_capture(t, set_at)? {
                Some(c) => cmd.captures.push(c),
                None => {
                    cmd.args.push(t.clone());
                    cmd.args_mask.push(mask.clone());
                    cmd.args_quoted
                        .push(super::word_quoted::word_quoted(t, *ts, *te));
                }
            }
        } else if let Some((r, extra)) = super::here_string::parse_here_string(tokens, i)? {
            bg_redirect::insert_redirect(&mut cmd.redirects, r)?;
            skip = extra;
        } else if let Some(r) = crate::parse::classify::parse_redirect(t, *fq)? {
            bg_redirect::insert_redirect(&mut cmd.redirects, r)?;
        } else {
            cmd.args.push(t.clone());
            cmd.args_mask.push(mask.clone());
            cmd.args_quoted
                .push(super::word_quoted::word_quoted(t, *ts, *te));
        }
        i += 1 + skip;
    }
    Ok(cmd)
}
