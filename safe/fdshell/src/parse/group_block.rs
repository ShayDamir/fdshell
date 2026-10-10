use super::Token;
use super::classify::parse_redirect;
use super::semi::closing_keyword_index;
use super::semi::verbatim;
use crate::error::parse::ParseError;
use crate::redirect::RedirectDef;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail, ensure};
use sys::ScriptText;

/// The `{ …; }` group command (POSIX #1.2): the body text to run in the
/// current shell, and the redirections written after the closing brace.
#[cfg_attr(test, derive(Debug))]
pub struct GroupBlock {
    pub body: ScriptText,
    pub redirects: Vec<RedirectDef>,
}

/// Parse the group tokens: the body between the braces, the required `;` (or
/// newline) before `}`, and the trailing redirections.
pub(crate) fn tokens_to_group(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<GroupBlock, Report<ParseError>> {
    ensure!(
        tokens
            .first()
            .is_some_and(|(t, _, _, _, _)| t.eq_bytes(b"{")),
        ParseError::MalformedGroupBlock
    );
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let close = closing_keyword_index(tokens, line).ok_or(ParseError::GroupMissingCloseBrace)?;
    let body_tokens = tokens.get(1..close).ok_or(ParseError::Never)?;
    ensure!(
        body_tokens.iter().any(|(t, _, _, _, _)| !t.eq_bytes(b";")),
        ParseError::GroupEmpty
    );
    // POSIX #1.2: `}` must be preceded by `;` or a newline. The tokenizer
    // emits a `;` token for a newline, so `{ echo a` newline `}` is accepted.
    // `closing_keyword_index` skips token 0 (the `{`), so `close` is never 0.
    ensure!(
        tokens
            .get(close.saturating_sub(1))
            .is_some_and(|(t, _, _, _, _)| t.eq_bytes(b";")),
        ParseError::ExpectedSemicolonBeforeCloseBrace
    );
    // The required `;` stays inside the body span: a `<<` operator's body lines
    // sit between it and the operator, so trimming the trailing side would cut
    // them out of the body text (the block-text-carries-its-bodies rule).
    let body = verbatim(text, body_tokens)?;
    let redirects = tail_redirects(&split_close(tokens, close))?;
    Ok(GroupBlock { body, redirects })
}

/// The redirections written after `}`, in source order. A here-doc/here-string
/// operator is checked first (it starts with `<`, which the read redirect would
/// otherwise swallow); a pipeline, background, here-doc, or here-string form on
/// the group is a parse error (each is a separate task), and so is a trailing
/// plain word.
fn tail_redirects(tail: &[Token]) -> Result<Vec<RedirectDef>, Report<ParseError>> {
    let mut redirects = Vec::new();
    let mut i = 0;
    while i < tail.len() {
        let (t, _, _, _, _) = tail.get(i).ok_or(ParseError::Never)?;
        if t.starts_with(b"<<") {
            bail!(ParseError::GroupHereDocNotSupported);
        }
        if let Some((def, extra)) = parse_redirect(tail, i)? {
            redirects.push(def);
            i += extra + 1;
            continue;
        }
        if t.eq_bytes(b"|") {
            bail!(ParseError::UnexpectedPipe);
        }
        if t.starts_with(b"&") {
            bail!(ParseError::UnexpectedChar { ch: b'&' });
        }
        bail!(ParseError::GroupTrailingWord);
    }
    Ok(redirects)
}

/// The tokens after the closing brace, with `}` split off the glued form
/// (`}>out3` gives the `>out3` token, aligned with its raw range and quote
/// mask), so a glued redirect parses as the group's redirection.
fn split_close(tokens: &[Token], close: usize) -> Vec<Token> {
    let tail = tokens.get(close + 1..).unwrap_or(&[]).to_vec();
    let Some((t, start, end, fq, mask)) = tokens.get(close).cloned() else {
        return tail;
    };
    if t.eq_bytes(b"}") {
        return tail;
    }
    let Some(rest) = t.get(1..) else {
        return tail;
    };
    let mut out = vec![(
        rest,
        start + 1,
        end,
        fq,
        mask.get(1..).unwrap_or(&[]).to_vec(),
    )];
    out.extend_from_slice(&tail);
    out
}

#[cfg(test)]
mod tests;
