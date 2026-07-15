use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(crate) fn find_preceded_by_semi(
    tokens: &[(ShortCStr, usize, bool)],
    start: usize,
    needle: &[u8],
) -> Option<usize> {
    let mut i = start;
    while i < tokens.len() {
        let preceded = i > 0 && tokens.get(i - 1).is_some_and(|(p, _, _)| p.eq_bytes(b";"));
        if tokens.get(i).is_some_and(|(t, _, _)| t.eq_bytes(needle)) && preceded {
            return Some(i);
        }
        i += 1;
    }
    None
}

pub(crate) fn trim_semi(tokens: &[(ShortCStr, usize, bool)]) -> &[(ShortCStr, usize, bool)] {
    let start = tokens
        .iter()
        .take_while(|(t, _, _)| t.eq_bytes(b";"))
        .count();
    let end = tokens
        .iter()
        .rev()
        .take_while(|(t, _, _)| t.eq_bytes(b";"))
        .count();
    let end = tokens.len().saturating_sub(end);
    tokens.get(start..end).unwrap_or(&[])
}

pub(crate) fn try_join(
    tokens: &[(ShortCStr, usize, bool)],
) -> Result<ShortCStr, Report<ParseError>> {
    let mut out = Vec::new();
    for (t, _, _) in tokens {
        if !out.is_empty() {
            out.push(b' ');
        }
        out.extend_from_slice(t.as_bytes().change_context(ParseError::Never)?);
    }
    let result = ShortCStr::from_vec(out).change_context(ParseError::Never)?;
    Ok(result)
}
