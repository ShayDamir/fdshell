use super::Token;
use super::cond_bodies::condition_bodies;
use super::semi::closing_keyword_index;
use super::semi::find_preceded_by_semi;
use super::semi::trim_semi;
use super::semi::verbatim;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::ScriptText;
use sys::ShortCStr;

pub struct ElifArm {
    pub cond: ScriptText,
    pub body: ScriptText,
    /// The condition's heredoc bodies (in operator order; empty when none).
    pub cond_bodies: Vec<ShortCStr>,
}

pub struct IfBlock {
    pub condition: ScriptText,
    pub then_body: ScriptText,
    pub elifs: Vec<ElifArm>,
    pub else_body: Option<ScriptText>,
    /// The condition's heredoc bodies (in operator order; empty when none).
    pub cond_bodies: Vec<ShortCStr>,
}

pub(crate) fn tokens_to_if(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<IfBlock, Report<ParseError>> {
    ensure!(
        tokens
            .first()
            .is_some_and(|(t, _, _, _, _)| t.eq_bytes(b"if")),
        ParseError::MalformedIfBlock
    );

    // A condition-position heredoc's bodies sit in the block text after the
    // closing `fi`: the tokens after it are body data, not block structure.
    let fi_idx = closing_keyword_index(tokens).ok_or(ParseError::MissingFi)?;
    let tokens = tokens.get(..=fi_idx).ok_or(ParseError::MissingFi)?;

    let first_then = find_preceded_by_semi(tokens, 1, b"then");
    let first_then = match first_then {
        Some(idx) => idx,
        None => return Err(ParseError::MissingThen.into()),
    };

    let cond_tokens = trim_semi(
        tokens
            .get(1..first_then)
            .ok_or(ParseError::MissingCondition)?,
    );

    let condition = span_verbatim(text, tokens, 1, first_then, ParseError::MissingCondition)?;
    let cond_bodies = condition_bodies(text, cond_tokens);

    let mut elif_pairs: Vec<(usize, usize)> = Vec::new();
    let mut pos = first_then;
    while let Some(elif_idx) = find_preceded_by_semi(tokens, pos, b"elif") {
        let then_idx = find_preceded_by_semi(tokens, elif_idx, b"then")
            .ok_or(ParseError::MissingThenAfterElif)?;
        elif_pairs.push((elif_idx, then_idx));
        pos = then_idx;
    }
    let else_idx = find_preceded_by_semi(tokens, pos, b"else");

    let first_end = match elif_pairs.first() {
        Some(&(ei, _)) => ei,
        None => else_idx.unwrap_or(fi_idx),
    };
    let then_body = span_verbatim(
        text,
        tokens,
        first_then + 1,
        first_end - 1,
        ParseError::MissingThen,
    )?;

    let elifs = super::elif::parse_elifs(tokens, &elif_pairs, else_idx, fi_idx, text)?;
    let else_body: Result<Option<ScriptText>, Report<ParseError>> = else_idx
        .map(|ei| super::elif::parse_else_body(tokens, ei, fi_idx, text))
        .transpose();
    let else_body = else_body?.filter(|t| !t.data.is_empty());
    Ok(IfBlock {
        condition,
        then_body,
        elifs,
        else_body,
        cond_bodies,
    })
}

/// The verbatim text of `tokens[from..to]`; `err` when the range is absent.
fn span_verbatim(
    text: &ScriptText,
    tokens: &[Token],
    from: usize,
    to: usize,
    err: ParseError,
) -> Result<ScriptText, Report<ParseError>> {
    verbatim(text, trim_semi(tokens.get(from..to).ok_or(err)?))
}
