use super::Token;
use crate::error::parse::ParseError;
use error_stack::Report;
use sys::ScriptText;
use sys::ShortCStr;

/// The index of the block's closing keyword: the token at which the block
/// depth (starting at 1 for the opening keyword) first returns to 0. Tokens
/// after it are heredoc body data (a condition-position heredoc's bodies sit
/// in the block text after the closing keyword), not block structure.
pub(crate) fn closing_keyword_index(tokens: &[Token], line: &[u8]) -> Option<usize> {
    let mut depth: i32 = 1;
    for (i, (t, _, end, _, _)) in tokens.iter().enumerate().skip(1) {
        let Some(word) = t.as_bytes().ok() else {
            continue;
        };
        if let Some(delta) = crate::keywords::keyword_delta(word, line, *end) {
            depth += delta;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

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
