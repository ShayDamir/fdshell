use crate::error::parse::ParseError;
use crate::parse::semi::{find_preceded_by_semi, trim_semi, try_join};
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug))]
pub struct ForBlock {
    pub var: ShortCStr,
    pub words: Vec<ShortCStr>,
    pub body: ShortCStr,
}

pub(crate) fn tokens_to_for(tokens: &[(ShortCStr, usize)]) -> Result<ForBlock, ParseError> {
    if !tokens.first().is_some_and(|(t, _)| t.eq_bytes(b"for")) {
        return Err(ParseError::Reason {
            pos: 0,
            reason: "expected 'for'",
        });
    }
    let var = tokens
        .get(1)
        .ok_or(ParseError::Reason {
            pos: 0,
            reason: "expected variable name",
        })?
        .0
        .clone();
    let in_pos = tokens
        .iter()
        .skip(2)
        .position(|(t, _)| t.eq_bytes(b"in"))
        .ok_or(ParseError::Reason {
            pos: 0,
            reason: "expected 'in'",
        })?
        + 2;

    let do_idx = find_preceded_by_semi(tokens, in_pos + 1, b"do").ok_or(ParseError::Reason {
        pos: 0,
        reason: "expected 'do'",
    })?;

    let done_idx = tokens.len() - 1;
    if !tokens.last().is_some_and(|(t, _)| t.eq_bytes(b"done")) {
        return Err(ParseError::Reason {
            pos: 0,
            reason: "expected 'done'",
        });
    }
    if !tokens
        .get(done_idx - 1)
        .is_some_and(|(t, _)| t.eq_bytes(b";"))
    {
        return Err(ParseError::Reason {
            pos: 0,
            reason: "expected ';' before 'done'",
        });
    }

    let body = try_join(trim_semi(tokens.get(do_idx + 1..done_idx - 1).ok_or(
        ParseError::Reason {
            pos: 0,
            reason: "expected 'done'",
        },
    )?))?;

    let word_tokens = trim_semi(
        tokens
            .get(in_pos + 1..do_idx - 1)
            .ok_or(ParseError::Reason {
                pos: 0,
                reason: "expected word list",
            })?,
    );
    let words: Vec<ShortCStr> = word_tokens.iter().map(|(t, _)| t.clone()).collect();

    Ok(ForBlock { var, words, body })
}
