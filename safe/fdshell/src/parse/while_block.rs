use crate::error::parse::ParseError;
use crate::parse::semi::{find_preceded_by_semi, trim_semi, try_join};
use error_stack::{Report, ensure};
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug))]
pub struct LoopBlock {
    pub condition: ShortCStr,
    pub body: ShortCStr,
}

pub type WhileBlock = LoopBlock;
pub type UntilBlock = LoopBlock;

pub(crate) fn tokens_to_loop(
    tokens: &[(ShortCStr, usize, bool)],
    keyword: &[u8],
) -> Result<LoopBlock, Report<ParseError>> {
    if !tokens.first().is_some_and(|(t, _, _)| t.eq_bytes(keyword)) {
        return Err(ParseError::Never.into());
    }

    let do_idx = find_preceded_by_semi(tokens, 1, b"do").ok_or(ParseError::ExpectedDo)?;
    ensure!(do_idx >= 2, ParseError::ExpectedCondition);

    let done_idx = tokens.len() - 1;
    ensure!(
        tokens.last().is_some_and(|(t, _, _)| t.eq_bytes(b"done")),
        ParseError::ExpectedDone
    );

    let cond_str = try_join(trim_semi(
        tokens.get(1..do_idx - 1).ok_or(ParseError::ExpectedDo)?,
    ))?;

    let body = try_join(trim_semi(
        tokens
            .get(do_idx + 1..done_idx - 1)
            .ok_or(ParseError::ExpectedDone)?,
    ))?;

    Ok(LoopBlock {
        condition: cond_str,
        body,
    })
}
