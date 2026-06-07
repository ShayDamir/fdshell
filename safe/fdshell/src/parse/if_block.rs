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

    let mut elifs = Vec::with_capacity(elif_pairs.len());
    for i in 0..elif_pairs.len() {
        let &(ei, ti) = elif_pairs.get(i).ok_or(EINVAL)?;
        let ec = try_join(trim_semi(tokens.get(ei + 1..ti - 1).ok_or(EINVAL)?))?;
        let next = elif_pairs
            .get(i + 1)
            .map(|&(ne, _)| ne)
            .or(else_idx)
            .unwrap_or(fi_idx);
        let eb = try_join(trim_semi(tokens.get(ti + 1..next - 1).ok_or(EINVAL)?))?;
        elifs.push((ec, eb));
    }

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

fn find_preceded_by_semi(tokens: &[ShortCStr], start: usize, needle: &[u8]) -> Option<usize> {
    let mut i = start;
    while i < tokens.len() {
        if tokens.get(i).is_some_and(|t| t.eq_bytes(needle))
            && tokens.get(i - 1).is_some_and(|p| p.eq_bytes(b";"))
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn trim_semi(tokens: &[ShortCStr]) -> &[ShortCStr] {
    let start = tokens
        .iter()
        .position(|t| !t.eq_bytes(b";"))
        .unwrap_or(tokens.len());
    let end = tokens
        .iter()
        .rposition(|t| !t.eq_bytes(b";"))
        .map(|p| p + 1)
        .unwrap_or(start);
    tokens.get(start..end).unwrap_or(&[])
}

fn try_join(tokens: &[ShortCStr]) -> Result<ShortCStr, i32> {
    let mut s = ShortCStr::new();
    for t in tokens {
        if !s.is_empty() {
            s.push(b' ')?;
        }
        for &b in t.as_bytes()? {
            s.push(b)?;
        }
    }
    Ok(s)
}
