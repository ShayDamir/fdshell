use sys::ShortCStr;
use sys::errno::EINVAL;

pub fn tokenize(line: &[u8]) -> Result<Vec<ShortCStr>, i32> {
    let mut tokens = Vec::new();
    let mut cur = ShortCStr::new();
    let mut in_quotes = false;
    let mut bytes = line.iter().copied().peekable();

    while let Some(b) = bytes.next() {
        if in_quotes {
            match b {
                b'"' => in_quotes = false,
                b'\\' => match bytes.next() {
                    Some(c) => cur.push(c)?,
                    None => return Err(EINVAL),
                },
                _ => cur.push(b)?,
            }
        } else {
            match b {
                b' ' | b'\t' => {
                    if !cur.is_empty() {
                        tokens.push(core::mem::take(&mut cur));
                    }
                }
                b'|' => {
                    if cur.starts_with(b"%") && cur.ends_with(b">")
                        || cur.starts_with(b"&") && cur.ends_with(b">")
                    {
                        cur.push(b'|')?;
                    } else {
                        if !cur.is_empty() {
                            tokens.push(core::mem::take(&mut cur));
                        }
                        tokens.push(c"|".into());
                    }
                }
                b';' | b'\n' => {
                    if !cur.is_empty() {
                        tokens.push(core::mem::take(&mut cur));
                    }
                    tokens.push(c";".into());
                }
                b'"' => in_quotes = true,
                b'$' if bytes.peek() == Some(&b'(') => {
                    super::token_subst::read_dollar_paren(&mut bytes, &mut cur)?;
                }
                b'`' => {
                    super::token_subst::read_backtick(&mut bytes, &mut cur)?;
                }
                _ => cur.push(b)?,
            }
        }
    }

    if in_quotes {
        return Err(EINVAL);
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    Ok(tokens)
}
