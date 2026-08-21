use crate::error::parse::ParseError;
use crate::redirect::RedirectDef;
use alloc::vec;
use error_stack::{Report, bail};
use sys::ShortCStr;

/// Parse a here-string token: `<<<word`, or a bare `<<<` taking the next token as its word.
/// Quoted tokens are never here-strings.
pub fn parse_here_string(
    t: &ShortCStr,
    fq: bool,
    iter: &mut core::iter::Peekable<core::slice::Iter<'_, ShortCStr>>,
    fq_iter: &mut vec::IntoIter<bool>,
) -> Result<Option<RedirectDef>, Report<ParseError>> {
    if fq || !t.starts_with(b"<<<") {
        return Ok(None);
    }
    // A bare `<<<` at end of input means an empty word: the tokenizer drops
    // empty quoted strings, so `<<<""` and `<<<` tokenize identically.
    let word = if t.len() > 3 {
        t.get(3..).ok_or(ParseError::Never)?
    } else {
        match iter.next() {
            Some(next)
                if next.starts_with(b"<")
                    || next.starts_with(b">")
                    || next.starts_with(b"&")
                    || next.starts_with(b"%") =>
            {
                bail!(ParseError::InvalidRedirect);
            }
            Some(next) => {
                let _ = fq_iter.next();
                next.clone()
            }
            None => ShortCStr::new(),
        }
    };
    Ok(Some(RedirectDef::here_string(word)))
}
