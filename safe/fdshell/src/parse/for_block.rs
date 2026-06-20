use crate::error::parse::{ParseError, report_error};
use crate::parse::semi::{find_preceded_by_semi, trim_semi, try_join};
use error_stack::Report;
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug))]
pub struct ForBlock {
    pub var: ShortCStr,
    pub words: Vec<ShortCStr>,
    pub body: ShortCStr,
}

pub(crate) fn tokens_to_for(tokens: &[(ShortCStr, usize)]) -> Result<ForBlock, Report<ParseError>> {
    if !tokens.first().is_some_and(|(t, _)| t.eq_bytes(b"for")) {
        return Err(report_error("expected 'for'", 0));
    }
    let var = tokens
        .get(1)
        .ok_or_else(|| report_error("expected variable name", 0))?
        .0
        .clone();
    let in_pos = tokens
        .iter()
        .skip(2)
        .position(|(t, _)| t.eq_bytes(b"in"))
        .ok_or_else(|| report_error("expected 'in'", 0))?
        + 2;

    let do_idx = find_preceded_by_semi(tokens, in_pos + 1, b"do")
        .ok_or_else(|| report_error("expected 'do'", 0))?;

    let done_idx = tokens.len() - 1;
    if !tokens.last().is_some_and(|(t, _)| t.eq_bytes(b"done")) {
        return Err(report_error("expected 'done'", 0));
    }
    if !tokens
        .get(done_idx - 1)
        .is_some_and(|(t, _)| t.eq_bytes(b";"))
    {
        return Err(report_error("expected ';' before 'done'", 0));
    }

    let body = try_join(trim_semi(
        tokens
            .get(do_idx + 1..done_idx - 1)
            .ok_or_else(|| report_error("expected 'done'", 0))?,
    ))?;

    let word_tokens = trim_semi(
        tokens
            .get(in_pos + 1..do_idx - 1)
            .ok_or_else(|| report_error("expected word list", 0))?,
    );
    let words: Vec<ShortCStr> = word_tokens.iter().map(|(t, _)| t.clone()).collect();

    Ok(ForBlock { var, words, body })
}
