use crate::error::parse::ParseError;
use crate::parse::semi::{find_preceded_by_semi, trim_semi, try_join};
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
) -> Result<LoopBlock, ParseError> {
    if !tokens.first().is_some_and(|(t, _)| t.eq_bytes(keyword)) {
        let reason = match keyword {
            b"while" => "expected 'while'",
            b"until" => "expected 'until'",
            _ => "expected loop keyword",
        };
        return Err(ParseError::Reason { pos: 0, reason });
    }

    let do_idx = find_preceded_by_semi(tokens, 1, b"do").ok_or(ParseError::Reason {
        pos: 0,
        reason: "expected 'do'",
    })?;
    if do_idx < 2 {
        return Err(ParseError::Reason {
            pos: 0,
            reason: "expected condition",
        });
    }

    let done_idx = tokens.len() - 1;
    if !tokens.last().is_some_and(|(t, _)| t.eq_bytes(b"done")) {
        return Err(ParseError::Reason {
            pos: 0,
            reason: "expected 'done'",
        });
    }

    let cond_str = try_join(trim_semi(tokens.get(1..do_idx - 1).ok_or(
        ParseError::Reason {
            pos: 0,
            reason: "expected 'do'",
        },
    )?))?;

    let body = try_join(trim_semi(tokens.get(do_idx + 1..done_idx - 1).ok_or(
        ParseError::Reason {
            pos: 0,
            reason: "expected 'done'",
        },
    )?))?;

    Ok(LoopBlock {
        condition: cond_str,
        body,
    })
}
