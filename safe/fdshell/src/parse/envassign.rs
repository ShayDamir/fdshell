//! Leading `NAME=value` words on a command (POSIX 2.9.1 scoped assignments).

use super::Token;
use alloc::vec::Vec;
use sys::ShortCStr;

/// A `NAME=value` word: non-empty lhs, no `%` prefix (the same leniency as
/// the bare-assignment detection — no new name validation).
pub(crate) fn is_env_assign(word: &ShortCStr) -> bool {
    word.split_once_byte(b'=')
        .is_some_and(|(lhs, _)| !lhs.is_empty() && !lhs.starts_with(b"%"))
}

/// A non-assignment word up to the first `;` — a command word that a
/// leading assignment prefix would attach to.
pub(crate) fn has_command_word(tokens: &[Token]) -> bool {
    tokens
        .iter()
        .take_while(|(t, _, _, _, _)| !t.eq_bytes(b";"))
        .any(|(t, _, _, _, _)| !is_env_assign(t))
}

/// The number of leading `NAME=value` words before the command word.
pub(crate) fn prefix_len(tokens: &[Token]) -> usize {
    let mut i = 0usize;
    while assign_at(tokens, i).is_some() {
        i += 1;
    }
    i
}

/// The `NAME=value` word at `i`, if it is a leading assignment with a
/// command word that can still follow.
fn assign_at(tokens: &[Token], i: usize) -> Option<(ShortCStr, ShortCStr)> {
    let (t, _, _, _, _) = tokens.get(i)?;
    if t.eq_bytes(b";") {
        return None;
    }
    let next = tokens.get(i + 1)?;
    if next.0.eq_bytes(b";") {
        return None;
    }
    let (name, value) = t.split_once_byte(b'=')?;
    if name.is_empty() || name.starts_with(b"%") {
        return None;
    }
    Some((name, value))
}

/// The `NAME=value` words of `tokens[..end]` as `(name, value)` pairs; by
/// the parse invariant, every word there is an assignment word.
pub(crate) fn collect_range(tokens: &[Token], end: usize) -> Vec<(ShortCStr, ShortCStr)> {
    let mut out = Vec::new();
    for (t, _, _, _, _) in tokens.get(..end).unwrap_or(tokens) {
        if let Some((name, value)) = t.split_once_byte(b'=')
            && !name.is_empty()
            && !name.starts_with(b"%")
        {
            out.push((name, value));
        }
    }
    out
}

/// Every word of an all-assignment statement as `(name, value)` pairs
/// (up to the first `;`), or `None` when a non-assignment word is present.
pub(crate) fn collect_all(tokens: &[Token]) -> Option<Vec<(ShortCStr, ShortCStr)>> {
    let mut out = Vec::new();
    for (t, _, _, _, _) in tokens {
        if t.eq_bytes(b";") {
            break;
        }
        let (name, value) = t.split_once_byte(b'=')?;
        if name.is_empty() || name.starts_with(b"%") {
            return None;
        }
        out.push((name, value));
    }
    Some(out)
}
