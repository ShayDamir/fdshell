//! The shared tail of a simple command: every word from word 0 on, collected
//! by the loop `command_args` and `redirect_first` both drive — the redirect
//! operators, the `%` captures and the plain words.

use crate::error::parse::ParseError;
use crate::parse::bg_redirect::{insert_redirect, parse_bg_redirect};
use crate::parse::classify::{parse_capture, parse_redirect};
use crate::parse::here_string::parse_here_string;
use crate::parse::heredoc::{HeredocBody, is_operator, parse_operator};
use crate::parse::word_quoted::word_quoted;
use crate::parse::{CommandLine, Token};
use error_stack::{Report, bail};
use sys::Position;

/// Collect `tokens[i..]` into `cmd`. Tokens after the first `;` are the heredoc
/// body region: they belong to the body, not the command. `word0_slot` opens
/// the command slot, so the **first** plain word becomes the command word
/// (`>f g` runs `g` with the redirect, as bash does); with the slot closed
/// every plain word is an argument, exactly as before.
pub(crate) fn collect(
    cmd: &mut CommandLine,
    tokens: &[Token],
    mut i: usize,
    line: &[u8],
    specs: &[HeredocBody],
    set_at: Position,
    mut word0_slot: bool,
) -> Result<(), Report<ParseError>> {
    let mut spec_at = 0usize;
    while let Some(tok @ (t, ts, te, _fq, _mask, _spans)) = tokens.get(i) {
        if t.eq_bytes(b";") {
            break;
        }
        // A bare `&` token is a parse error here: the scanner consumes every
        // statement-terminating `&` (task #326), so one reaching the parse
        // layer is the empty-command family (`echo a && & echo b` — bash's
        // syntax error too). A quoted or escaped `&` is an ordinary argument
        // (`echo "&"` prints `&`, measured bash).
        if t.eq_bytes(b"&") && *te - *ts == t.len() {
            bail!(ParseError::UnexpectedChar { ch: b'&' });
        }
        let mut skip = 0usize;
        if is_operator(line, tokens, i) {
            let spec = specs.get(spec_at).ok_or(ParseError::InvalidRedirect)?;
            let (r, extra) = parse_operator(line, tokens, i, spec)?;
            insert_redirect(&mut cmd.redirects, r);
            spec_at += 1;
            skip = extra;
        } else if let Some((bg, extra)) = parse_bg_redirect(tokens, i)? {
            // A pidvar bg redirect carries no redirects, so the loop is a
            // no-op there; the two cases share one straight-line form.
            for r in bg.redirects {
                insert_redirect(&mut cmd.redirects, r);
            }
            if let Some(p) = bg.pidvar {
                cmd.pidvar = Some(p);
                cmd.bg_force = bg.bg_force;
            }
            skip = extra;
        } else if t.starts_with(b"%") {
            match parse_capture(t, set_at)? {
                Some(c) => cmd.captures.push(c),
                None => word(cmd, tok, &mut word0_slot),
            }
        } else if let Some((r, extra)) = parse_here_string(line, tokens, i)? {
            insert_redirect(&mut cmd.redirects, r);
            skip = extra;
        } else if let Some((r, extra)) = parse_redirect(tokens, i)? {
            insert_redirect(&mut cmd.redirects, r);
            skip = extra;
        } else {
            word(cmd, tok, &mut word0_slot);
        }
        i += 1 + skip;
    }
    Ok(())
}

/// One plain word: into the open command slot when there is one (word 0 of
/// `>f g` — its mask, spans and quoted flag are word 0's, as `finish_command`
/// builds them), into `args` otherwise.
fn word(cmd: &mut CommandLine, tok: &Token, slot: &mut bool) {
    let (t, ts, te, _fq, mask, spans) = tok;
    let quoted = word_quoted(t, *ts, *te);
    if *slot {
        cmd.command = t.clone();
        cmd.command_mask = mask.to_vec();
        cmd.command_span = spans.to_vec();
        cmd.command_quoted = quoted;
        *slot = false;
        return;
    }
    cmd.args.push(t.clone());
    cmd.args_mask.push(mask.to_vec());
    cmd.args_span.push(spans.to_vec());
    cmd.args_quoted.push(quoted);
}
