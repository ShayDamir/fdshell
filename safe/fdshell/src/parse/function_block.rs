use super::Token;
use super::semi::{trim_semi, verbatim};
use crate::error::parse::ParseError;
use crate::keywords::name_len;
use error_stack::Report;
use sys::{ScriptText, ShortCStr};

#[cfg_attr(test, derive(Debug))]
pub struct FunctionDef {
    pub name: ShortCStr,
    pub body: ScriptText,
}

/// A function-definition opener: a `{` token, and either the POSIX `name() {`
/// definer (`name(` then `)`) or the bash/ksh `function name [()] {` definer.
pub(crate) fn is_function_def(tokens: &[Token]) -> bool {
    let open = tokens.iter().any(|(t, _, _, _, _, _)| t.eq_bytes(b"{"));
    open && (posix_def(tokens) || keyword_def(tokens))
}

/// POSIX `name() {`: token 0 is `name(` with a non-empty name, token 1 is `)`.
fn posix_def(tokens: &[Token]) -> bool {
    let parens = tokens
        .get(1)
        .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b")"));
    parens
        && tokens
            .first()
            .is_some_and(|(t, _, _, _, _, _)| t.ends_with(b"(") && def_len(t).is_some())
}

/// bash/ksh `function name [()] {`: token 0 is `function`, token 1 is a definer name.
fn keyword_def(tokens: &[Token]) -> bool {
    let keyword = tokens
        .first()
        .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"function"));
    keyword
        && tokens
            .get(1)
            .is_some_and(|(t, _, _, _, _, _)| def_len(t).is_some())
}

/// The definer word: the name token after `function` in the keyword form, the
/// first token (`name(`) in the POSIX form.
fn definer(tokens: &[Token]) -> Option<&ShortCStr> {
    let first = tokens.first().map(|(t, _, _, _, _, _)| t);
    if first.is_some_and(|t| t.eq_bytes(b"function")) {
        return tokens.get(1).map(|(t, _, _, _, _, _)| t);
    }
    first
}

/// The definer name length of a token; `None` = not a definer word.
fn def_len(word: &ShortCStr) -> Option<usize> {
    name_len(word.as_bytes().ok()?)
}

pub(crate) fn tokens_to_function(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<FunctionDef, Report<ParseError>> {
    let name = function_name(tokens)?;
    // `is_function_def` guarantees an opening `{` exists, so this cannot fail.
    let open = tokens
        .iter()
        .position(|(t, _, _, _, _, _)| t.eq_bytes(b"{"))
        .ok_or(ParseError::Never)?;
    let close = tokens
        .iter()
        .rposition(|(t, _, _, _, _, _)| t.eq_bytes(b"}"))
        .ok_or(ParseError::FunctionMissingCloseBrace)?;
    let body_tokens = tokens
        .get(open + 1..close)
        .ok_or(ParseError::FunctionMissingCloseBrace)?;
    let body = verbatim(text, trim_semi(body_tokens))?;
    Ok(FunctionDef { name, body })
}

/// The definer word with its trailing POSIX `(` stripped.
fn function_name(tokens: &[Token]) -> Result<ShortCStr, Report<ParseError>> {
    let word = definer(tokens).ok_or(ParseError::FunctionEmptyName)?;
    let bytes = word.as_bytes().ok().ok_or(ParseError::FunctionEmptyName)?;
    let len = name_len(bytes).ok_or(ParseError::FunctionEmptyName)?;
    Ok(word.get(..len).ok_or(ParseError::FunctionEmptyName)?)
}

#[cfg(test)]
mod tests;
