use super::semi::{find_preceded_by_semi, trim_semi, try_join};
use crate::error::parse::ParseErrorInfo;
use sys::ShortCStr;

pub struct IfBlock {
    pub condition: ShortCStr,
    pub then_body: ShortCStr,
    pub elifs: Vec<(ShortCStr, ShortCStr)>,
    pub else_body: Option<ShortCStr>,
}

pub(crate) fn tokens_to_if(tokens: &[(ShortCStr, usize)]) -> Result<IfBlock, ParseErrorInfo> {
    if !tokens.first().is_some_and(|(t, _)| t.eq_bytes(b"if")) {
        return Err(ParseErrorInfo {
            source_start: 0,
            message: None,
        });
    }

    let if_pos = tokens.first().map(|(_, p)| *p).unwrap_or(0);

    let first_then = find_preceded_by_semi(tokens, 1, b"then");
    let first_then = match first_then {
        Some(idx) => idx,
        None => {
            return Err(ParseErrorInfo::new(if_pos, "missing 'then'"));
        }
    };

    // first_then >= 2 is guaranteed by find_preceded_by_semi starting at index 1
    // and requiring a preceding ';' (which can't be at index 0 since that's 'if').
    // The condition is empty only if first_then == 2 (tokens: [if, ;, then, ...]).

    let fi_idx = tokens.len() - 1;
    if !tokens.last().is_some_and(|(t, _)| t.eq_bytes(b"fi")) {
        return Err(ParseErrorInfo::new(if_pos, "missing 'fi'"));
    }

    let cond_str =
        try_join(trim_semi(tokens.get(1..first_then - 1).ok_or_else(
            || ParseErrorInfo::new(if_pos, "missing condition"),
        )?))
        .map_err(ParseErrorInfo::from)?;

    let mut elif_pairs: Vec<(usize, usize)> = Vec::new();
    let mut pos = first_then + 1;
    while let Some(elif_idx) = find_preceded_by_semi(tokens, pos, b"elif") {
        let then_idx = find_preceded_by_semi(tokens, elif_idx + 1, b"then").ok_or_else(|| {
            let elif_pos = tokens.get(elif_idx).map(|(_, p)| *p).unwrap_or(0);
            ParseErrorInfo::new(elif_pos, "missing 'then' after 'elif'")
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
            .ok_or_else(|| ParseErrorInfo::new(if_pos, "missing 'then'"))?,
    ))
    .map_err(ParseErrorInfo::from)?;
    // Empty then body is accepted (valid: `if true; then; fi`)

    let elifs = elif_pairs
        .iter()
        .enumerate()
        .map(|(i, &(ei, ti))| {
            let ec = try_join(trim_semi(tokens.get(ei + 1..ti - 1).ok_or_else(|| {
                ParseErrorInfo::new(
                    tokens.get(ei).map(|(_, p)| *p).unwrap_or(0),
                    "missing condition",
                )
            })?))
            .map_err(ParseErrorInfo::from)?;
            let next = elif_pairs
                .get(i + 1)
                .map(|&(ne, _)| ne)
                .or(else_idx)
                .unwrap_or(fi_idx);
            let eb = try_join(trim_semi(tokens.get(ti + 1..next - 1).ok_or_else(
                || {
                    ParseErrorInfo::new(
                        tokens.get(ti).map(|(_, p)| *p).unwrap_or(0),
                        "missing 'then'",
                    )
                },
            )?))
            .map_err(ParseErrorInfo::from)?;
            Ok((ec, eb))
        })
        .collect::<Result<Vec<_>, ParseErrorInfo>>()?;

    let else_str = else_idx
        .map(|ei| {
            try_join(trim_semi(tokens.get(ei + 1..fi_idx - 1).ok_or_else(
                || {
                    ParseErrorInfo::new(
                        tokens.get(ei).map(|(_, p)| *p).unwrap_or(0),
                        "missing 'else' body",
                    )
                },
            )?))
            .map_err(ParseErrorInfo::from)
        })
        .transpose()?;
    Ok(IfBlock {
        condition: cond_str,
        then_body: then_str,
        elifs,
        else_body: else_str.filter(|s| !s.is_empty()),
    })
}
