//! Word-pattern detection and single-component pattern matching.

pub(in crate::glob) mod bracket;

use crate::mask::MaskBit;

/// True when `bytes` (parallel quote `mask`) holds an unquoted, unescaped
/// `*` or `?`, or an unquoted `[` opening a valid bracket expression.
/// Quoted bytes never count; unquoted `\X` pairs are literal.
pub(crate) fn has_unquoted_pattern(bytes: &[u8], mask: &[MaskBit]) -> bool {
    let mut i = 0usize;
    while let Some(&b) = bytes.get(i) {
        if bracket::quoted(mask, i) {
            i += 1;
        } else {
            match b {
                b'*' | b'?' => return true,
                b'\\' => i += 2,
                b'[' if bracket::span(bytes, mask, i).is_some() => return true,
                _ => i += 1,
            }
        }
    }
    false
}

/// Match one path-component `name` against pattern component `pat`.
///
/// Iterative star-backtracking, no recursion (LESSONS.md). `*` never crosses
/// `/` because components are matched separately; a leading unquoted `*`, `?`
/// or valid `[...]` cannot consume a name's leading `.` (bash FNM_PERIOD)
/// unless `dotglob` is set.
///
/// Word patterns (`${name#pat}` and friends) pass `dotglob = true`: a word
/// pattern has no `FNM_PERIOD` rule, so `d=.abc` with `${d#*c}` strips the
/// whole value — measured against bash 5.3.9, where the pathname glob with
/// `dotglob` off refuses the leading dot.
pub(crate) fn match_component(pat: &[u8], mask: &[MaskBit], name: &[u8], dotglob: bool) -> bool {
    if dot_blocked(pat, mask, name, dotglob) {
        return false;
    }
    let mut pi = 0usize;
    let mut ni = 0usize;
    let mut star: Option<(usize, usize)> = None;
    loop {
        let next = pat.get(pi).and_then(|&pc| {
            if bracket::quoted(mask, pi) {
                (name.get(ni) == Some(&pc)).then(|| (pi + 1, ni + 1))
            } else {
                match pc {
                    b'*' => {
                        // The resume point is **one** value: the record and the
                        // forward step must be the same byte, so a mutant of it
                        // cannot hide in the sibling copy (it then re-reads the
                        // star forever, which the suite sees as a hang).
                        let resume = (pi + 1, ni);
                        star = Some(resume);
                        Some(resume)
                    }
                    b'?' => name.get(ni).is_some().then(|| (pi + 1, ni + 1)),
                    b'\\' => match pat.get(pi + 1) {
                        Some(&nx) => (name.get(ni) == Some(&nx)).then(|| (pi + 2, ni + 1)),
                        None => (name.get(ni) == Some(&b'\\')).then(|| (pi + 1, ni + 1)),
                    },
                    b'[' => match bracket::span(pat, mask, pi) {
                        Some(end) => name.get(ni).and_then(|&nc| {
                            bracket::contains(pat, mask, pi, nc).then_some((end, ni + 1))
                        }),
                        // Unclosed `[`: literal byte (FNM-style fallback).
                        None => (name.get(ni) == Some(&b'[')).then(|| (pi + 1, ni + 1)),
                    },
                    _ => (name.get(ni) == Some(&pc)).then(|| (pi + 1, ni + 1)),
                }
            }
        });
        match (next, star) {
            (Some((p, n)), _) => {
                pi = p;
                ni = n;
            }
            // A failed step (or an exhausted pattern) backtracks one name
            // byte to the last star, which then eats it. The star's own
            // position needs no test: it is recorded with `sn = ni`, forward
            // steps only raise `ni`, and an extension sets `ni = sn + 1` = the
            // new `sn`, so `ni >= sn` and the name test below is the whole
            // rule (a second `sn` bound is a duplicate whose mutants are
            // unkillable). A star past its end and a fully consumed name
            // decide the verdict below without re-exploring.
            (None, Some((sp, sn))) if ni < name.len() => {
                star = Some((sp, sn + 1));
                pi = sp;
                ni = sn + 1;
            }
            // No extendable star: the match holds iff pattern and name are
            // both exactly consumed.
            _ => return ni == name.len() && pat.get(pi).is_none(),
        }
    }
}

/// A name starting with `.` needs a literal first pattern byte: a leading
/// unquoted `*`, `?`, or valid `[...]` cannot consume it — unless `dotglob`
/// is set, in which case those pattern bytes may eat the leading `.`.
fn dot_blocked(pat: &[u8], mask: &[MaskBit], name: &[u8], dotglob: bool) -> bool {
    if name.first() != Some(&b'.') || dotglob {
        return false;
    }
    match pat.first().copied() {
        Some(b'*' | b'?') => !bracket::quoted(mask, 0),
        Some(b'[') => !bracket::quoted(mask, 0) && bracket::span(pat, mask, 0).is_some(),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
