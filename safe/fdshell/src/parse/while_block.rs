use crate::parse::semi::{find_preceded_by_semi, trim_semi, try_join};
use sys::ShortCStr;
use sys::errno::EINVAL;

#[cfg_attr(test, derive(Debug))]
pub struct LoopBlock {
    pub condition: ShortCStr,
    pub body: ShortCStr,
}

pub type WhileBlock = LoopBlock;
pub type UntilBlock = LoopBlock;

pub(crate) fn tokens_to_loop(tokens: &[ShortCStr], keyword: &[u8]) -> Result<LoopBlock, i32> {
    if !tokens.first().is_some_and(|t| t.eq_bytes(keyword)) {
        return Err(EINVAL);
    }

    let do_idx = find_preceded_by_semi(tokens, 1, b"do").ok_or(EINVAL)?;
    if do_idx < 2 {
        return Err(EINVAL);
    }

    let done_idx = tokens.len() - 1;
    if !tokens.last().is_some_and(|t| t.eq_bytes(b"done")) {
        return Err(EINVAL);
    }

    let cond_str = try_join(trim_semi(tokens.get(1..do_idx - 1).ok_or(EINVAL)?))?;

    let body = try_join(trim_semi(
        tokens.get(do_idx + 1..done_idx - 1).ok_or(EINVAL)?,
    ))?;

    Ok(LoopBlock {
        condition: cond_str,
        body,
    })
}
