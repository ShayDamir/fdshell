//! Leading `NAME=value` words on a command (POSIX 2.9.1 scoped assignments).

use super::Token;
use crate::mask::MaskBit;
use crate::parse::word_quoted::span_word;
use alloc::vec::Vec;
use sys::ShortCStr;

/// One raw `NAME=value` word: the name, the raw (unexpanded) value word, and
/// the value's per-byte quote mask — the token's word mask sliced at the `=`.
/// The mask travels with the value so every substitution site expands the value
/// as its own quoting says (task #217), never with an empty mask. `spans` is
/// the parallel zero-byte quote-span list, sliced at the same offset (task #321).
#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub struct Assign {
    pub name: ShortCStr,
    pub value: ShortCStr,
    pub mask: Vec<MaskBit>,
    pub spans: Vec<usize>,
}

/// The value's mask: the word mask sliced at the `=` (offset `name.len() + 1`).
/// This is the **only** place that offset exists, so the three assignment
/// surfaces cannot drift apart (LESSONS: slice the word, its mask and its
/// quoted flags with the same offset). A mask shorter than the value degrades
/// to unquoted, the same padding `substitute/arg.rs` applies.
pub fn value_mask(word_mask: &[MaskBit], name_len: usize) -> Vec<MaskBit> {
    word_mask.get(name_len + 1..).unwrap_or_default().to_vec()
}

/// The value's zero-byte quote spans (task #321): sliced at the same
/// `name.len() + 1` offset as [`value_mask`] and shifted down to value indices,
/// so the tilde gate reads the value's own coordinates.
pub fn value_spans(word_spans: &[usize], name_len: usize) -> Vec<usize> {
    let off = name_len + 1;
    word_spans
        .iter()
        .filter(|&&i| i >= off)
        .map(|&i| i - off)
        .collect()
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
/// leading assignment prefix would attach to. The `;` is [`span_word`]'s, so a
/// quoted `";"` is the command word it is (`X=1 ";"` runs the command `;`).
pub(crate) fn has_command_word(tokens: &[Token]) -> bool {
    tokens
        .iter()
        .take_while(|tok| !span_word(tok, b";"))
        .any(|(t, _, _, _, _, _)| !is_env_assign(t))
}

/// The number of leading `NAME=value` words before the command word.
///
/// The walk is bounded by the statement itself — the first `;`, or the end of
/// the tokens — so the bound is computed once, here, and the last word of the
/// statement is never a prefix word: it has no word behind it to become the
/// command. The bound is the only place that rule lives; a per-word "is the
/// next word `;`" lookahead is the same rule written twice, and its offset
/// mutants are unkillable (LESSONS: "The scoped-prefix parse bounds are
/// load-bearing").
pub(crate) fn prefix_len(tokens: &[Token]) -> usize {
    let stmt_end = tokens
        .iter()
        .take_while(|tok| !span_word(tok, b";"))
        .count();
    let mut i = 0usize;
    while i + 1 < stmt_end && assign_word(tokens, i).is_some() {
        i += 1;
    }
    i
}

/// The `NAME=value` word at `i`: `None` past the slice end or for a word that
/// is no assignment. The caller's statement bound is what keeps a `;` and the
/// statement's last word out of the prefix.
fn assign_word(tokens: &[Token], i: usize) -> Option<Assign> {
    let (t, _, _, _, mask, spans) = tokens.get(i)?;
    assign_of(t, mask, spans)
}

/// The `Assign` for one word plus its word mask and zero-byte span list:
/// `None` when the word is not an assignment word. The value's mask and spans
/// are sliced here, once, by `value_mask` and `value_spans`.
fn assign_of(word: &ShortCStr, word_mask: &[MaskBit], word_spans: &[usize]) -> Option<Assign> {
    let (name, value) = word.split_once_byte(b'=')?;
    if name.is_empty() || name.starts_with(b"%") || name.contains(b'\\') {
        return None;
    }
    let mask = value_mask(word_mask, name.len());
    let spans = value_spans(word_spans, name.len());
    Some(Assign {
        name,
        value,
        mask,
        spans,
    })
}

/// The `NAME=value` words of `tokens[..end]` as `Assign`es; by the parse
/// invariant, every word there is an assignment word.
pub(crate) fn collect_range(tokens: &[Token], end: usize) -> Vec<Assign> {
    tokens
        .get(..end)
        .unwrap_or(tokens)
        .iter()
        .filter_map(|(t, _, _, _, mask, spans)| assign_of(t, mask, spans))
        .collect()
}

/// Every word of an all-assignment statement as `Assign`es (up to the first
/// `;`), or `None` when a non-assignment word is present.
pub(crate) fn collect_all(tokens: &[Token]) -> Option<Vec<Assign>> {
    let mut out = Vec::new();
    for tok @ (t, _, _, _, mask, spans) in tokens {
        if span_word(tok, b";") {
            break;
        }
        out.push(assign_of(t, mask, spans)?);
    }
    Some(out)
}

#[cfg(test)]
mod tests;
