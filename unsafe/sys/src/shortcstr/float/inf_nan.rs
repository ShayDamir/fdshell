//! The `inf`/`infinity` and `nan([payload])` words of float parsing.

/// `inf`/`infinity` and `nan([payload])`, any case; `None` otherwise.
pub(super) fn inf_nan(rest: &[u8]) -> Option<(f64, usize)> {
    let ci = |s: &[u8], t: &[u8]| {
        s.len() >= t.len() && s.get(..t.len()).is_some_and(|p| p.eq_ignore_ascii_case(t))
    };
    if ci(rest, b"inf") {
        // "inf" (3) or "infinity" (8): the extra 5 bytes are present or not.
        let n = 3 + usize::from(ci(rest, b"infinity")) * 5;
        return Some((f64::INFINITY, n));
    }
    if ci(rest, b"nan") {
        let n = rest
            .get(3..)
            .filter(|s| s.first() == Some(&b'('))
            .and_then(|s| s.iter().position(|&c| c == b')').map(|close| 4 + close))
            .unwrap_or(3);
        return Some((f64::NAN, n));
    }
    None
}
