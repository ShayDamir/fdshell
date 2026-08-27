use super::Token;
use super::if_block::ElifArm;
use super::semi::trim_semi;
use super::semi::verbatim;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, bail};
use sys::ScriptText;

pub fn parse_elifs(
    tokens: &[Token],
    elif_pairs: &[(usize, usize)],
    else_idx: Option<usize>,
    fi_idx: usize,
    text: &ScriptText,
) -> Result<Vec<ElifArm>, Report<ParseError>> {
    elif_pairs
        .iter()
        .enumerate()
        .map(|(i, &(ei, ti))| {
            let cond = tokens
                .get(ei + 1..ti - 1)
                .ok_or(ParseError::MissingCondition)?;
            if cond.last().is_some_and(|(t, _, _, _, _)| t.eq_bytes(b";")) {
                bail!(ParseError::MalformedIfBlock);
            }
            let ec = verbatim(text, trim_semi(cond))?;
            let next = elif_pairs
                .get(i + 1)
                .map(|&(ne, _)| ne)
                .or(else_idx)
                .unwrap_or(fi_idx);
            let body = tokens
                .get(ti + 1..next - 1)
                .ok_or(ParseError::MissingThen)?;
            if body.last().is_some_and(|(t, _, _, _, _)| t.eq_bytes(b";")) {
                bail!(ParseError::MalformedIfBlock);
            }
            let eb = verbatim(text, trim_semi(body))?;
            Ok(ElifArm { cond: ec, body: eb })
        })
        .collect::<Result<Vec<_>, Report<ParseError>>>()
}

pub fn parse_else_body(
    tokens: &[Token],
    else_idx: usize,
    fi_idx: usize,
    text: &ScriptText,
) -> Result<ScriptText, Report<ParseError>> {
    let raw = tokens.get(else_idx + 1..fi_idx - 1).unwrap_or(&[]);
    if raw.last().is_some_and(|(t, _, _, _, _)| t.eq_bytes(b";")) {
        bail!(ParseError::MalformedIfBlock);
    }
    let trimmed = trim_semi(raw);
    verbatim(text, trimmed)
}
