use crate::error::parse::ParseError;
use sys::ShortCStr;

pub fn tokenize(line: &[u8]) -> Result<Vec<(ShortCStr, usize)>, ParseError> {
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
            match b {
                b'"' => in_quotes = false,
                b'\\' => {
                    if let Some(c) = bytes.next() {
                        cur.push(c)?;
                    } else {
                        return Err(ParseError::UnexpectedEof { pos });
                    }
                }
                _ => cur.push(b)?,
            }
        } else {
            match b {
                b' ' | b'\t' => {
                    if !cur.is_empty() {
                        tokens.push((core::mem::take(&mut cur), token_start));
                    }
                    token_start = pos;
                }
                b'|' => {
                    if cur.starts_with(b"%") && cur.ends_with(b">")
                        || cur.starts_with(b"&") && cur.ends_with(b">")
                    {
                        cur.push(b'|')?;
                    } else {
                        if !cur.is_empty() {
                            tokens.push((core::mem::take(&mut cur), token_start));
                        }
                        tokens.push((c"|".into(), pos - 1));
                    }
                }
                b';' | b'\n' => {
                    if !cur.is_empty() {
                        tokens.push((core::mem::take(&mut cur), token_start));
                    }
                    tokens.push((c";".into(), pos - 1));
                    token_start = pos;
                }
                b'"' => {
                    in_quotes = true;
                    quote_start = Some(pos - 1);
                }
                b'$' => {
                    if bytes.peek() == Some(&b'(') {
                        let start = pos - 1; // position of '$'
                        super::token_subst::read_dollar_paren(&mut cur, &mut bytes, start)?;
                    } else {
                        cur.push(b)?;
                    }
                }
                b'`' => {
                    let start = pos - 1; // position of '`'
                    super::token_subst::read_backtick(&mut cur, &mut bytes, start)?;
                }
                _ => cur.push(b)?,
            }
        }
    }

    if in_quotes {
        return Err(ParseError::UnbalancedQuote {
            pos: quote_start.unwrap_or(0),
        });
    }
    if !cur.is_empty() {
        tokens.push((cur, token_start));
    }
    Ok(tokens)
}
