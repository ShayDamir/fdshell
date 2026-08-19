use super::comment::skip_comment;
use super::emit::emit_token;
use super::token_pipe::handle_pipe;
use crate::error::parse::{ParseError, report_unbalanced_quote};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub fn tokenize(line: &[u8]) -> Result<Vec<(ShortCStr, usize, bool)>, Report<ParseError>> {
    let mut tokens = Vec::new();
    let mut cur = ShortCStr::new();
    let mut in_quotes = false;
    let mut quote_start: Option<usize> = None;
    let mut token_fully_quoted = false;
    let mut bytes = line.iter().copied().peekable();
    let mut pos = 0usize;
    let mut token_start = 0usize;
    while let Some(b) = bytes.next() {
        pos += 1;
        if in_quotes {
            if !super::quotes::handle_quoted_char(b, &mut cur, &mut bytes, line, &mut pos)? {
                in_quotes = false;
                quote_start = None;
            }
        } else {
            match b {
                b' ' | b'\t' | b';' | b'\n' | b')' => {
                    let needs_sep = b == b';' || b == b'\n' || b == b')';
                    let sep = if b == b')' { c")" } else { c";" };
                    emit_token(&mut tokens, &mut cur, token_start, token_fully_quoted);
                    token_fully_quoted = false;
                    token_start = pos;
                    if needs_sep {
                        tokens.push((sep.into(), pos - 1, false));
                    }
                }
                b'|' => {
                    if handle_pipe(&mut tokens, &mut cur, token_start, token_fully_quoted, pos)? {
                        token_fully_quoted = false;
                    }
                }
                b'"' => {
                    if cur.is_empty() {
                        token_fully_quoted = true;
                    }
                    in_quotes = true;
                    quote_start = Some(pos - 1);
                }
                b'$' if bytes.peek() == Some(&b'(') => {
                    token_fully_quoted = false;
                    super::token_subst::read_dollar_paren(line, &mut cur, &mut bytes, &mut pos)?;
                }
                b'`' => {
                    token_fully_quoted = false;
                    super::backtick::read_backtick(line, &mut cur, &mut bytes, &mut pos)?;
                }
                b'#' => {
                    let consumed = skip_comment(&mut bytes);
                    pos += consumed - 1;
                    emit_token(&mut tokens, &mut cur, token_start, token_fully_quoted);
                    token_fully_quoted = false;
                    token_start = pos;
                }
                _ => {
                    token_fully_quoted = false;
                    cur.push_byte(b)
                        .change_context(ParseError::InvalidChar { ch: 0 })?;
                }
            }
        }
    }
    if in_quotes {
        return Err(report_unbalanced_quote(line, quote_start.unwrap_or(0)));
    }
    if !cur.is_empty() {
        tokens.push((cur, token_start, token_fully_quoted));
    }
    Ok(tokens)
}
