use super::semi::{trim_semi, try_join};
use crate::error::parse::{ParseError, report_error};
use error_stack::Report;
use sys::ShortCStr;

pub fn parse_elifs(
    tokens: &[(ShortCStr, usize, bool)],
    elif_pairs: &[(usize, usize)],
    else_idx: Option<usize>,
    fi_idx: usize,
) -> Result<Vec<(ShortCStr, ShortCStr)>, Report<ParseError>> {
    elif_pairs
        .iter()
        .enumerate()
        .map(|(i, &(ei, ti))| {
            let ec = try_join(trim_semi(tokens.get(ei + 1..ti - 1).ok_or_else(|| {
                let p = tokens.get(ei).map(|(_, p, _)| *p).unwrap_or(0);
                report_error("missing condition", p)
            })?))?;
            let next = elif_pairs
                .get(i + 1)
                .map(|&(ne, _)| ne)
                .or(else_idx)
                .unwrap_or(fi_idx);
            let eb = try_join(trim_semi(tokens.get(ti + 1..next - 1).ok_or_else(
                || {
                    let p = tokens.get(ti).map(|(_, p, _)| *p).unwrap_or(0);
                    report_error("missing 'then'", p)
                },
            )?))?;
            Ok((ec, eb))
        })
        .collect::<Result<Vec<_>, Report<ParseError>>>()
}

pub fn parse_else_body(
    tokens: &[(ShortCStr, usize, bool)],
    else_idx: usize,
    fi_idx: usize,
) -> Result<ShortCStr, Report<ParseError>> {
    try_join(trim_semi(tokens.get(else_idx + 1..fi_idx - 1).ok_or_else(
        || {
            let p = tokens.get(else_idx).map(|(_, p, _)| *p).unwrap_or(0);
            report_error("missing 'else' body", p)
        },
    )?))
}
