mod types;

pub(crate) use types::{ElifArm, IfBlock};

use super::Token;
use super::cond_bodies::condition_bodies;
use super::part_text::part_text;
use super::semi::List;
use super::semi::block_body;
use super::semi::find_preceded_by_semi;
use super::semi::trim_semi;
use super::semi::up_to_closing;
use super::semi::verbatim;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, ensure};
use sys::ScriptText;

pub(crate) fn tokens_to_if(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<IfBlock, Report<ParseError>> {
    ensure!(
        tokens
            .first()
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"if")),
        ParseError::MalformedIfBlock
    );

    // A condition-position heredoc's bodies sit in the block text after the
    // closing `fi`: the tokens after it are body data, not block structure.
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let tokens = up_to_closing(tokens, line, List::Plain, ParseError::MissingFi)?;
    // The closing `fi` is the truncated slice's last token.
    let fi_idx = tokens.len() - 1;

    let first_then =
        find_preceded_by_semi(tokens, 1, b"then", line).ok_or(ParseError::MissingThen)?;

    let cond_tokens = trim_semi(
        tokens
            .get(1..first_then)
            .ok_or(ParseError::MissingCondition)?,
    );

    let condition = span_verbatim(text, tokens, 1, first_then, ParseError::MissingCondition)?;
    let cond_bodies = condition_bodies(text, cond_tokens);

    let mut elif_pairs: Vec<(usize, usize)> = Vec::new();
    let mut pos = first_then;
    while let Some(elif_idx) = find_preceded_by_semi(tokens, pos, b"elif", line) {
        let then_idx = find_preceded_by_semi(tokens, elif_idx, b"then", line)
            .ok_or(ParseError::MissingThenAfterElif)?;
        elif_pairs.push((elif_idx, then_idx));
        pos = then_idx;
    }
    let else_idx = find_preceded_by_semi(tokens, pos, b"else", line);

    let first_end = match elif_pairs.first() {
        Some(&(ei, _)) => ei,
        None => else_idx.unwrap_or(fi_idx),
    };
    // `first_end - 1` drops the `;` before the next keyword (POSIX requires it);
    // a body glued to the closer has no token left here, so it is an empty body.
    let raw = tokens.get(first_then + 1..first_end - 1).unwrap_or(&[]);
    let then_body = part_text(text, block_body(raw, ParseError::EmptyThenBody)?)?;

    let elifs = super::elif::parse_elifs(tokens, &elif_pairs, else_idx, fi_idx, text)?;
    let else_body = else_idx
        .map(|ei| super::elif::parse_else_body(tokens, ei, fi_idx, text))
        .transpose()?;
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
