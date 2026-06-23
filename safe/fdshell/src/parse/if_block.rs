use super::semi::find_preceded_by_semi;
use super::semi::{trim_semi, try_join};
use crate::error::parse::{ParseError, report_error};
use error_stack::Report;
use sys::ShortCStr;

pub struct IfBlock {
    pub condition: ShortCStr,
    pub then_body: ShortCStr,
    pub elifs: Vec<(ShortCStr, ShortCStr)>,
    pub else_body: Option<ShortCStr>,
}

pub(crate) fn tokens_to_if(tokens: &[(ShortCStr, usize)]) -> Result<IfBlock, Report<ParseError>> {
    if !tokens.first().is_some_and(|(t, _)| t.eq_bytes(b"if")) {
        return Err(report_error("malformed if block", 0));
    }

    let if_pos = tokens.first().map(|(_, p)| *p).unwrap_or(0);

    let first_then = find_preceded_by_semi(tokens, 1, b"then");
    let first_then = match first_then {
        Some(idx) => idx,
        None => return Err(report_error("missing 'then'", if_pos)),
    };

    let fi_idx = tokens.len() - 1;
    if !tokens.last().is_some_and(|(t, _)| t.eq_bytes(b"fi")) {
        return Err(report_error("missing 'fi'", if_pos));
    }

    let cond_str = try_join(trim_semi(
        tokens
            .get(1..first_then - 1)
            .ok_or_else(|| report_error("missing condition", if_pos))?,
    ))?;

    let mut elif_pairs: Vec<(usize, usize)> = Vec::new();
    let mut pos = first_then + 1;
    while let Some(elif_idx) = find_preceded_by_semi(tokens, pos, b"elif") {
        let then_idx = find_preceded_by_semi(tokens, elif_idx + 1, b"then").ok_or_else(|| {
            let elif_pos = tokens.get(elif_idx).map(|(_, p)| *p).unwrap_or(0);
            report_error("missing 'then' after 'elif'", elif_pos)
        })?;
        elif_pairs.push((elif_idx, then_idx));
        pos = then_idx + 1;
    }
    let else_idx = find_preceded_by_semi(tokens, pos, b"else");

    let first_end = elif_pairs
        .first()
        .map(|&(ei, _)| ei)
        .or(else_idx)
        .unwrap_or(fi_idx);
    let then_str = try_join(trim_semi(
        tokens
            .get(first_then + 1..first_end - 1)
            .ok_or_else(|| report_error("missing 'then'", if_pos))?,
    ))?;

    let elifs = super::elif::parse_elifs(tokens, &elif_pairs, else_idx, fi_idx)?;
    let else_str = else_idx
        .map(|ei| super::elif::parse_else_body(tokens, ei, fi_idx))
        .transpose()?;
    Ok(IfBlock {
        condition: cond_str,
        then_body: then_str,
        elifs,
        else_body: else_str.filter(|s| !s.is_empty()),
    })
}
