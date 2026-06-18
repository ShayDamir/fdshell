use crate::error::parse::ParseError;
use sys::ShortCStr;

pub(crate) fn read_dollar_paren(
    cur: &mut ShortCStr,
    bytes: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
    start: usize,
) -> Result<(), ParseError> {
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
            None => {
                return Err(ParseError::UnexpectedEof { pos: start });
            }
        }
    }
    Ok(())
}

pub(crate) fn read_backtick(
    cur: &mut ShortCStr,
    bytes: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
    start: usize,
) -> Result<(), ParseError> {
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
                None => {
                    return Err(ParseError::UnexpectedEof { pos: start });
                }
            },
            Some(c) => cur.push(c)?,
            None => {
                return Err(ParseError::UnexpectedEof { pos: start });
            }
        }
    }
}
