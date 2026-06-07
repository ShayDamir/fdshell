use crate::parse::semi::{find_preceded_by_semi, trim_semi, try_join};
use sys::ShortCStr;
use sys::errno::EINVAL;

#[cfg_attr(test, derive(Debug))]
pub struct ForBlock {
    pub var: ShortCStr,
    pub words: Vec<ShortCStr>,
    pub body: ShortCStr,
}

pub(crate) fn tokens_to_for(tokens: &[ShortCStr]) -> Result<ForBlock, i32> {
    if !tokens.first().is_some_and(|t| t.eq_bytes(b"for")) {
        return Err(EINVAL);
    }
    let var = tokens.get(1).ok_or(EINVAL)?.clone();

    let in_pos = tokens
        .iter()
        .enumerate()
        .skip(2)
        .find(|(_, t)| t.eq_bytes(b"in"))
        .ok_or(EINVAL)?
        .0;

    let do_idx = find_preceded_by_semi(tokens, in_pos + 1, b"do").ok_or(EINVAL)?;

    let done_idx = tokens.len() - 1;
    if !tokens.last().is_some_and(|t| t.eq_bytes(b"done")) {
        return Err(EINVAL);
    }
    if !tokens.get(done_idx - 1).is_some_and(|t| t.eq_bytes(b";")) {
        return Err(EINVAL);
    }

    let body = try_join(trim_semi(
        tokens.get(do_idx + 1..done_idx - 1).ok_or(EINVAL)?,
    ))?;

    let word_tokens = trim_semi(tokens.get(in_pos + 1..do_idx - 1).ok_or(EINVAL)?);
    let words: Vec<ShortCStr> = word_tokens.to_vec();

    Ok(ForBlock { var, words, body })
}
