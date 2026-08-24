use super::Token;
use super::semi::{trim_semi, verbatim};
use crate::error::parse::ParseError;
use error_stack::Report;
use sys::{ScriptText, ShortCStr};

#[cfg_attr(test, derive(Debug))]
pub struct FunctionDef {
    pub name: ShortCStr,
    pub body: ScriptText,
}

/// A `name() { … }` opener: the first token is `name(`, the second is `)`,
/// and a `{` token follows.
pub(crate) fn is_function_def(tokens: &[Token]) -> bool {
    let first = tokens
        .first()
        .is_some_and(|(t, _, _, _)| t.len() > 1 && t.ends_with(b"("));
    let second = tokens.get(1).is_some_and(|(t, _, _, _)| t.eq_bytes(b")"));
    let open = tokens.iter().any(|(t, _, _, _)| t.eq_bytes(b"{"));
    first && second && open
}

pub(crate) fn tokens_to_function(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<FunctionDef, Report<ParseError>> {
    let name = function_name(tokens)?;
    // `is_function_def` guarantees an opening `{` exists, so this cannot fail.
    let open = tokens
        .iter()
        .position(|(t, _, _, _)| t.eq_bytes(b"{"))
        .ok_or(ParseError::Never)?;
    let close = tokens
        .iter()
        .rposition(|(t, _, _, _)| t.eq_bytes(b"}"))
        .ok_or(ParseError::FunctionMissingCloseBrace)?;
    let body_tokens = tokens
        .get(open + 1..close)
        .ok_or(ParseError::FunctionMissingCloseBrace)?;
    let body = verbatim(text, trim_semi(body_tokens))?;
    Ok(FunctionDef { name, body })
}

/// The `name(` first token with its trailing `(` stripped.
fn function_name(tokens: &[Token]) -> Result<ShortCStr, Report<ParseError>> {
    let head = tokens
        .first()
        .ok_or(ParseError::FunctionEmptyName)?
        .0
        .clone();
    let name = head
        .get(..head.len() - 1)
        .ok_or(ParseError::FunctionEmptyName)?;
    Ok(name)
}

#[cfg(test)]
mod tests;
