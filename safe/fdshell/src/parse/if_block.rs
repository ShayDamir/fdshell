use super::semi::{find_preceded_by_semi, trim_semi, try_join};
use sys::ShortCStr;
use sys::errno::EINVAL;

pub struct IfBlock {
    pub condition: ShortCStr,
    pub then_body: ShortCStr,
    pub elifs: Vec<(ShortCStr, ShortCStr)>,
    pub else_body: Option<ShortCStr>,
}

pub(crate) fn tokens_to_if(tokens: &[ShortCStr]) -> Result<IfBlock, i32> {
    if !tokens.first().is_some_and(|t| t.eq_bytes(b"if")) {
        return Err(EINVAL);
    }
    let first_then = find_preceded_by_semi(tokens, 1, b"then").ok_or(EINVAL)?;
    if first_then < 2 {
        return Err(EINVAL);
    }
    let fi_idx = tokens.len() - 1;
    if !tokens.last().is_some_and(|t| t.eq_bytes(b"fi")) {
        return Err(EINVAL);
    }

    let cond_str = try_join(trim_semi(tokens.get(1..first_then - 1).ok_or(EINVAL)?))?;

    let mut elif_pairs: Vec<(usize, usize)> = Vec::new();
    let mut pos = first_then + 1;
    while let Some(elif_idx) = find_preceded_by_semi(tokens, pos, b"elif") {
        let then_idx = find_preceded_by_semi(tokens, elif_idx + 1, b"then").ok_or(EINVAL)?;
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
        tokens.get(first_then + 1..first_end - 1).ok_or(EINVAL)?,
    ))?;

    let elifs = elif_pairs
        .iter()
        .enumerate()
        .map(|(i, &(ei, ti))| {
            let ec = try_join(trim_semi(tokens.get(ei + 1..ti - 1).ok_or(EINVAL)?))?;
            let next = elif_pairs
                .get(i + 1)
                .map(|&(ne, _)| ne)
                .or(else_idx)
                .unwrap_or(fi_idx);
            let eb = try_join(trim_semi(tokens.get(ti + 1..next - 1).ok_or(EINVAL)?))?;
            Ok((ec, eb))
        })
        .collect::<Result<Vec<_>, i32>>()?;

    let else_str = else_idx
        .map(|ei| try_join(trim_semi(tokens.get(ei + 1..fi_idx - 1).ok_or(EINVAL)?)))
        .transpose()?;
    Ok(IfBlock {
        condition: cond_str,
        then_body: then_str,
        elifs,
        else_body: else_str.filter(|s| !s.is_empty()),
    })
}
