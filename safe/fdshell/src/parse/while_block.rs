use crate::error::parse::{ParseError, report_error};
use crate::parse::semi::{find_preceded_by_semi, trim_semi, try_join};
use error_stack::Report;
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug))]
pub struct LoopBlock {
    pub condition: ShortCStr,
    pub body: ShortCStr,
}

pub type WhileBlock = LoopBlock;
pub type UntilBlock = LoopBlock;

pub(crate) fn tokens_to_loop(
    tokens: &[(ShortCStr, usize)],
    keyword: &[u8],
) -> Result<LoopBlock, Report<ParseError>> {
    if !tokens.first().is_some_and(|(t, _)| t.eq_bytes(keyword)) {
        let reason = match keyword {
            b"while" => "expected 'while'",
            b"until" => "expected 'until'",
            _ => "expected loop keyword",
        };
        return Err(report_error(reason, 0));
    }

    let do_idx =
        find_preceded_by_semi(tokens, 1, b"do").ok_or_else(|| report_error("expected 'do'", 0))?;
    if do_idx < 2 {
        return Err(report_error("expected condition", 0));
    }

    let done_idx = tokens.len() - 1;
    if !tokens.last().is_some_and(|(t, _)| t.eq_bytes(b"done")) {
        return Err(report_error("expected 'done'", 0));
    }

    let cond_str = try_join(trim_semi(
        tokens
            .get(1..do_idx - 1)
            .ok_or_else(|| report_error("expected 'do'", 0))?,
    ))?;

    let body = try_join(trim_semi(
        tokens
            .get(do_idx + 1..done_idx - 1)
            .ok_or_else(|| report_error("expected 'done'", 0))?,
    ))?;

    Ok(LoopBlock {
        condition: cond_str,
        body,
    })
}
