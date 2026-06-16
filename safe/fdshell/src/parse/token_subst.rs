use crate::error::parse::ParseErrorInfo;
use sys::ShortCStr;

pub(crate) fn read_dollar_paren(
    cur: &mut ShortCStr,
    bytes: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
) -> Result<(), ParseErrorInfo> {
    cur.push(b'$')?;
    cur.push(b'(')?;
    bytes.next(); // consume '('
    let mut depth = 1u32;
    while depth > 0 {
        match bytes.next() {
            Some(b'(') => {
                cur.push(b'(')?;
                depth += 1;
            }
            Some(b')') => {
                depth -= 1;
                if depth == 0 {
                    cur.push(b')')?;
                    break;
                }
                cur.push(b')')?;
            }
            Some(c) => cur.push(c)?,
            None => return Err(ParseErrorInfo { source_start: 0 }),
        }
    }
    Ok(())
}

pub(crate) fn read_backtick(
    cur: &mut ShortCStr,
    bytes: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
) -> Result<(), ParseErrorInfo> {
    cur.push(b'`')?;
    loop {
        match bytes.next() {
            Some(b'`') => {
                cur.push(b'`')?;
                return Ok(());
            }
            Some(b'\\') => match bytes.next() {
                Some(b'`') => cur.push(b'`')?,
                Some(b'\\') => {
                    cur.push(b'\\')?;
                    cur.push(b'\\')?;
                }
                Some(c) => {
                    cur.push(b'\\')?;
                    cur.push(c)?;
                }
                None => return Err(ParseErrorInfo { source_start: 0 }),
            },
            Some(c) => cur.push(c)?,
            None => return Err(ParseErrorInfo { source_start: 0 }),
        }
    }
}
