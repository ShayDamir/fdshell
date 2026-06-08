use sys::ShortCStr;
use sys::errno::EINVAL;

pub(crate) fn read_dollar_paren(
    bytes: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
    cur: &mut ShortCStr,
) -> Result<(), i32> {
    cur.push(b'$')?;
    cur.push(b'(')?;
    bytes.next();
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
            None => return Err(EINVAL),
        }
    }
    Ok(())
}

pub(crate) fn read_backtick(
    bytes: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
    cur: &mut ShortCStr,
) -> Result<(), i32> {
    cur.push(b'`')?;
    loop {
        match bytes.next() {
            Some(b'`') => {
                cur.push(b'`')?;
                return Ok(());
            }
            Some(b'\\') => match bytes.next() {
                Some(b'`') => cur.push(b'`')?,
                Some(b'\\') => cur.push(b'\\')?,
                Some(c) => {
                    cur.push(b'\\')?;
                    cur.push(c)?;
                }
                None => return Err(EINVAL),
            },
            Some(c) => cur.push(c)?,
            None => return Err(EINVAL),
        }
    }
}
