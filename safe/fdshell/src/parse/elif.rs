use super::Token;
use super::cond_bodies::condition_bodies;
use super::if_block::ElifArm;
use super::part_text::part_text;
use super::semi::block_body;
use super::semi::body_end;
use super::semi::trim_condition;
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
            // The token before a clause keyword is POSIX's `;` and drops out; a
            // plain `&` stays (task #354: `elif true & then …`, and a body whose
            // last statement the `&` backgrounds).
            let cond = tokens
                .get(ei + 1..body_end(tokens, ti))
                .ok_or(ParseError::MissingCondition)?;
            if cond
                .last()
                .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
            {
                bail!(ParseError::MalformedIfBlock);
            }
            let cond_trimmed = trim_condition(cond);
            let ec = verbatim(text, cond_trimmed)?;
            let cond_bodies = condition_bodies(text, cond_trimmed);
            let next = elif_pairs
                .get(i + 1)
                .map(|&(ne, _)| ne)
                .or(else_idx)
                .unwrap_or(fi_idx);
            let raw = tokens.get(ti + 1..body_end(tokens, next)).unwrap_or(&[]);
            if raw
                .last()
                .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
            {
                bail!(ParseError::MalformedIfBlock);
            }
            let eb = part_text(text, block_body(raw, ParseError::EmptyElifBody)?)?;
            Ok(ElifArm {
                cond: ec,
                body: eb,
                cond_bodies,
            })
        })
        .collect::<Result<Vec<_>, Report<ParseError>>>()
}

pub fn parse_else_body(
    tokens: &[Token],
    else_idx: usize,
    fi_idx: usize,
    text: &ScriptText,
) -> Result<ScriptText, Report<ParseError>> {
    let raw = tokens
        .get(else_idx + 1..body_end(tokens, fi_idx))
        .unwrap_or(&[]);
    if raw
        .last()
        .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
    {
        bail!(ParseError::MalformedIfBlock);
    }
    part_text(text, block_body(raw, ParseError::EmptyElseBody)?)
}
