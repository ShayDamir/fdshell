use sys::ShortCStr;

pub(crate) fn find_preceded_by_semi(
    tokens: &[(ShortCStr, usize)],
    start: usize,
    needle: &[u8],
) -> Option<usize> {
    let mut i = start;
    while i < tokens.len() {
        if tokens.get(i).is_some_and(|(t, _)| t.eq_bytes(needle))
            && tokens.get(i - 1).is_some_and(|(p, _)| p.eq_bytes(b";"))
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

pub(crate) fn trim_semi(tokens: &[(ShortCStr, usize)]) -> &[(ShortCStr, usize)] {
    let start = tokens
        .iter()
        .position(|(t, _)| !t.eq_bytes(b";"))
        .unwrap_or(tokens.len());
    let end = tokens
        .iter()
        .rposition(|(t, _)| !t.eq_bytes(b";"))
        .map(|p| p + 1)
        .unwrap_or(start);
    tokens.get(start..end).unwrap_or(&[])
}

pub(crate) fn try_join(tokens: &[(ShortCStr, usize)]) -> Result<ShortCStr, i32> {
    let mut s = ShortCStr::new();
    for (t, _) in tokens {
        if !s.is_empty() {
            s.push(b' ')?;
        }
        for &b in t.as_bytes()? {
            s.push(b)?;
        }
    }
    Ok(s)
}
