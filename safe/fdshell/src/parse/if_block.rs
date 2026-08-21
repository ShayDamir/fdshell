use super::Token;
use super::semi::find_preceded_by_semi;
use super::semi::trim_semi;
use super::semi::verbatim;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::ScriptText;

pub struct ElifArm {
    pub cond: ScriptText,
    pub body: ScriptText,
}

pub struct IfBlock {
    pub condition: ScriptText,
    pub then_body: ScriptText,
    pub elifs: Vec<ElifArm>,
    pub else_body: Option<ScriptText>,
}

pub(crate) fn tokens_to_if(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<IfBlock, Report<ParseError>> {
    ensure!(
        tokens.first().is_some_and(|(t, _, _, _)| t.eq_bytes(b"if")),
        ParseError::MalformedIfBlock
    );

    let first_then = find_preceded_by_semi(tokens, 1, b"then");
    let first_then = match first_then {
        Some(idx) => idx,
        None => return Err(ParseError::MissingThen.into()),
    };

    let fi_idx = tokens.len() - 1;
    ensure!(
        tokens.last().is_some_and(|(t, _, _, _)| t.eq_bytes(b"fi")),
        ParseError::MissingFi
    );

    let condition = verbatim(
        text,
        trim_semi(
            tokens
                .get(1..first_then)
                .ok_or(ParseError::MissingCondition)?,
        ),
    )?;

    let mut elif_pairs: Vec<(usize, usize)> = Vec::new();
    let mut pos = first_then;
    while let Some(elif_idx) = find_preceded_by_semi(tokens, pos, b"elif") {
        let then_idx = find_preceded_by_semi(tokens, elif_idx, b"then")
            .ok_or(ParseError::MissingThenAfterElif)?;
        elif_pairs.push((elif_idx, then_idx));
        pos = then_idx;
    }
    let else_idx = find_preceded_by_semi(tokens, pos, b"else");

    let first_end = elif_pairs
        .first()
        .map(|&(ei, _)| ei)
        .or(else_idx)
        .unwrap_or(fi_idx);
    let then_body = verbatim(
        text,
        trim_semi(
            tokens
                .get(first_then + 1..first_end - 1)
                .ok_or(ParseError::MissingThen)?,
        ),
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
    })
}
