use super::comment::skip_comment;
use super::emit::emit_token;
use crate::error::parse::{ParseError, report_unbalanced_quote};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub fn tokenize(line: &[u8]) -> Result<Vec<(ShortCStr, usize, bool)>, Report<ParseError>> {
    let mut tokens = Vec::new();
    let mut cur = ShortCStr::new();
    let mut in_quotes = false;
    let mut quote_start: Option<usize> = None;
    let mut bytes = line.iter().copied().peekable();
    let mut pos = 0usize;
    let mut token_start = 0usize;

    while let Some(b) = bytes.next() {
        pos += 1;

        if in_quotes {
            if !super::quotes::handle_quoted_char(b, &mut cur, &mut bytes, line, pos)? {
                in_quotes = false;
                quote_start = None;
            }
        } else {
            match b {
                b' ' | b'\t' => {
                    emit_token(&mut tokens, &mut cur, token_start);
                    token_start = pos;
                }
                b'|' => {
                    if cur.starts_with(b"%") && cur.ends_with(b">")
                        || cur.starts_with(b"&") && cur.ends_with(b">")
                    {
                        cur.push(b'|')
                            .change_context(ParseError::InvalidChar { ch: 0 })?;
                    } else {
                        emit_token(&mut tokens, &mut cur, token_start);
                        tokens.push((c"|".into(), pos - 1, false));
                    }
                }
                b';' | b'\n' => {
                    emit_token(&mut tokens, &mut cur, token_start);
                    tokens.push((c";".into(), pos - 1, false));
                    token_start = pos;
                }
                b'"' => {
                    in_quotes = true;
                    quote_start = Some(pos - 1);
                }
                b'$' => {
                    if bytes.peek() == Some(&b'(') {
                        let start = pos - 1; // position of '$'
                        super::token_subst::read_dollar_paren(line, &mut cur, &mut bytes, start)?;
                    } else {
                        cur.push(b)
                            .change_context(ParseError::InvalidChar { ch: 0 })?;
                    }
                }
                b'`' => {
                    let start = pos - 1; // position of '`'
                    super::backtick::read_backtick(line, &mut cur, &mut bytes, start)?;
                }
                b'#' => {
                    let consumed = skip_comment(&mut bytes);
                    pos += consumed - 1; // -1 because outer loop increments pos once
                    emit_token(&mut tokens, &mut cur, token_start);
                    token_start = pos;
                }
                _ => cur
                    .push(b)
                    .change_context(ParseError::InvalidChar { ch: 0 })?,
            }
        }
    }

    if in_quotes {
        return Err(report_unbalanced_quote(line, quote_start.unwrap_or(0)));
    }
    if !cur.is_empty() {
        let fully_quoted = quote_start.is_some();
        tokens.push((cur, token_start, fully_quoted));
    }
    Ok(tokens)
}
