use crate::error::parse::ParseError;
use crate::parse::semi::{find_preceded_by_semi, trim_semi, try_join};
use error_stack::{Report, ensure};
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug))]
pub struct ForBlock {
    pub var: ShortCStr,
    pub words: Vec<ShortCStr>,
    pub body: ShortCStr,
}

pub(crate) fn tokens_to_for(
    tokens: &[(ShortCStr, usize, bool)],
) -> Result<ForBlock, Report<ParseError>> {
    ensure!(
        tokens.first().is_some_and(|(t, _, _)| t.eq_bytes(b"for")),
        ParseError::ExpectedFor
    );
    let var = tokens
        .get(1)
        .ok_or(ParseError::ExpectedVariableName)?
        .0
        .clone();
    let in_pos = tokens
        .iter()
        .skip(2)
        .position(|(t, _, _)| t.eq_bytes(b"in"))
        .ok_or(ParseError::ExpectedIn)?
        + 2;

    let do_idx = find_preceded_by_semi(tokens, in_pos + 1, b"do").ok_or(ParseError::ExpectedDo)?;

    let done_idx = tokens.len() - 1;
    ensure!(
        tokens.last().is_some_and(|(t, _, _)| t.eq_bytes(b"done")),
        ParseError::ExpectedDone
    );
    ensure!(
        tokens
            .get(done_idx - 1)
            .is_some_and(|(t, _, _)| t.eq_bytes(b";")),
        ParseError::ExpectedSemicolonBeforeDone
    );

    let body = try_join(trim_semi(
        tokens
            .get(do_idx + 1..done_idx - 1)
            .ok_or(ParseError::ExpectedDone)?,
    ))?;

    let word_tokens = trim_semi(
        tokens
            .get(in_pos + 1..do_idx - 1)
            .ok_or(ParseError::ExpectedWordList)?,
    );
    let words: Vec<ShortCStr> = word_tokens.iter().map(|(t, _, _)| t.clone()).collect();

    Ok(ForBlock { var, words, body })
}
