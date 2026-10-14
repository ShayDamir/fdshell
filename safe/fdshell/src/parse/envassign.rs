//! Leading `NAME=value` words on a command (POSIX 2.9.1 scoped assignments).

use super::Token;
use alloc::vec::Vec;
use sys::ShortCStr;

/// One raw `NAME=value` word: the name, the raw (unexpanded) value word, and
/// the value's per-byte quote mask — the token's word mask sliced at the `=`.
/// The mask travels with the value so every substitution site expands the value
/// as its own quoting says (task #217), never with an empty mask.
#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub struct Assign {
    pub name: ShortCStr,
    pub value: ShortCStr,
    pub mask: Vec<bool>,
}

/// The value's mask: the word mask sliced at the `=` (offset `name.len() + 1`).
/// This is the **only** place that offset exists, so the three assignment
/// surfaces cannot drift apart (LESSONS: slice the word, its mask and its
/// quoted flags with the same offset). A mask shorter than the value degrades
/// to unquoted, the same padding `substitute/arg.rs` applies.
pub fn value_mask(word_mask: &[bool], name_len: usize) -> Vec<bool> {
    word_mask.get(name_len + 1..).unwrap_or_default().to_vec()
}

/// A `NAME=value` word: non-empty lhs, no `%` prefix (the same leniency as
/// the bare-assignment detection — no new name validation), and no `\` in the
/// name (an escape pair is not a name byte: `\X=1` is a command word, not an
/// assignment, as in bash).
pub(crate) fn is_env_assign(word: &ShortCStr) -> bool {
    word.split_once_byte(b'=')
        .is_some_and(|(lhs, _)| !lhs.is_empty() && !lhs.starts_with(b"%") && !lhs.contains(b'\\'))
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
fn assign_at(tokens: &[Token], i: usize) -> Option<Assign> {
    let (t, _, _, _, mask) = tokens.get(i)?;
    if t.eq_bytes(b";") {
        return None;
    }
    let next = tokens.get(i + 1)?;
    if next.0.eq_bytes(b";") {
        return None;
    }
    assign_of(t, mask)
}

/// The `Assign` for one word plus its word mask: `None` when the word is not
/// an assignment word. The value's mask is sliced here, once, by `value_mask`.
fn assign_of(word: &ShortCStr, word_mask: &[bool]) -> Option<Assign> {
    let (name, value) = word.split_once_byte(b'=')?;
    if name.is_empty() || name.starts_with(b"%") || name.contains(b'\\') {
        return None;
    }
    let mask = value_mask(word_mask, name.len());
    Some(Assign { name, value, mask })
}

/// The `NAME=value` words of `tokens[..end]` as `Assign`es; by the parse
/// invariant, every word there is an assignment word.
pub(crate) fn collect_range(tokens: &[Token], end: usize) -> Vec<Assign> {
    tokens
        .get(..end)
        .unwrap_or(tokens)
        .iter()
        .filter_map(|(t, _, _, _, mask)| assign_of(t, mask))
        .collect()
}

/// Every word of an all-assignment statement as `Assign`es (up to the first
/// `;`), or `None` when a non-assignment word is present.
pub(crate) fn collect_all(tokens: &[Token]) -> Option<Vec<Assign>> {
    let mut out = Vec::new();
    for (t, _, _, _, mask) in tokens {
        if t.eq_bytes(b";") {
            break;
        }
        out.push(assign_of(t, mask)?);
    }
    Some(out)
}
