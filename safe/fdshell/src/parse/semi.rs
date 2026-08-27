use super::Token;
use crate::error::parse::ParseError;
use error_stack::Report;
use sys::ScriptText;
use sys::ShortCStr;

pub(crate) fn find_preceded_by_semi(
    tokens: &[Token],
    start: usize,
    needle: &[u8],
) -> Option<usize> {
    for (i, (t, _, _, _, _)) in tokens.iter().enumerate().skip(start) {
        let preceded = i > 0
            && tokens
                .get(i - 1)
                .is_some_and(|(p, _, _, _, _)| p.eq_bytes(b";"));
        if t.eq_bytes(needle) && preceded {
            return Some(i);
        }
    }
    None
}

pub(crate) fn trim_semi(tokens: &[Token]) -> &[Token] {
    let start = tokens
        .iter()
        .take_while(|(t, _, _, _, _)| t.eq_bytes(b";"))
        .count();
    let end = tokens
        .iter()
        .rev()
        .take_while(|(t, _, _, _, _)| t.eq_bytes(b";"))
        .count();
    let end = tokens.len().saturating_sub(end);
    tokens.get(start..end).unwrap_or(&[])
}

pub(crate) fn try_join(tokens: &[Token]) -> ShortCStr {
    let mut out = ShortCStr::new();
    for (t, _, _, _, _) in tokens {
        if !out.is_empty() {
            out.push(c" ");
        }
        out.push(t);
    }
    out
}

/// A verbatim subslice of `text` covering the given tokens' byte ranges.
pub(crate) fn verbatim(
    text: &ScriptText,
    tokens: &[Token],
) -> Result<ScriptText, Report<ParseError>> {
    let (off, end) = token_range(tokens);
    let t = text.subslice(off, end - off).ok_or(ParseError::Never)?;
    Ok(t)
}

/// Byte range `(start, end)` covered by the first and last tokens, or `(0, 0)`.
pub(crate) fn token_range(tokens: &[Token]) -> (usize, usize) {
    match (tokens.first(), tokens.last()) {
        (Some(f), Some(l)) => (f.1, l.2),
        _ => (0, 0),
    }
}
