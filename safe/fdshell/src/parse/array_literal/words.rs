//! The word-level predicates of the array arm (task #422): which word opens a
//! region, what its first element is, and when the whole statement is the arm's.
//! The assembly walk itself is [`super::detect`].

use alloc::vec::Vec;

use crate::mask::{MaskBit, quoted_at};
use crate::parse::Token;
use crate::parse::envassign;
use sys::ShortCStr;

use super::{ArrayAssign, ArrayWord};

/// The arm's decision at the statement's end: a completed region means the whole
/// statement was array-assignment words (`a=(1) b=(2)`, which bash answers
/// `1-2`); no completed region is the ordinary walk's.
pub(super) fn all_array(assigns: &[ArrayAssign]) -> Option<Vec<ArrayAssign>> {
    (!assigns.is_empty()).then(|| assigns.to_vec())
}

/// The `name=(` word that opens a region: an assignment word by `envassign`'s
/// one name predicate, whose value starts with an **unquoted** `(` — read from
/// the word's mask at that byte, never derived. The name may end with `+`.
pub(super) fn open_word(tok: &Token) -> Option<ArrayAssign> {
    let (text, _, _, fq, mask, spans) = tok;
    if !envassign::is_env_assign(text) {
        return None;
    }
    let (name, value) = text.split_once_byte(b'=')?;
    let open = name.len() + 1;
    if !value.starts_with(b"(") || quoted_at(mask, open) {
        return None;
    }
    let append = name.ends_with(b"+");
    let name = if append {
        name.get(..name.len() - 1)?
    } else {
        name
    };
    let mut assign = ArrayAssign {
        name,
        append,
        elements: Vec::new(),
    };
    first_element(&mut assign, value, mask, spans, *fq, open);
    Some(assign)
}

/// A later word of the region, kept whole: its own text, mask, spans and quoted
/// flag (the parallel plumbing again — the runtime expands it as its own word).
pub(super) fn push(
    assign: &mut ArrayAssign,
    text: &ShortCStr,
    mask: &[MaskBit],
    spans: &[usize],
    quoted: bool,
) {
    assign.elements.push(ArrayWord {
        text: text.clone(),
        mask: mask.to_vec(),
        spans: spans.to_vec(),
        quoted,
    });
}

/// The region's first element word: the value with its `(` stripped, the mask
/// and spans sliced at the existing `value_mask` / `value_spans` offset — the
/// `=`'s, plus the `(`. A word that carried no bytes and no quote material is no
/// element (`a=()` is bash's empty array); a `""` pair is one.
fn first_element(
    assign: &mut ArrayAssign,
    value: ShortCStr,
    mask: &[MaskBit],
    spans: &[usize],
    fq: bool,
    open: usize,
) {
    let Some(text) = value.get(1..) else {
        return;
    };
    if text.is_empty() && spans.is_empty() && !fq {
        return;
    }
    assign.elements.push(ArrayWord {
        text,
        mask: envassign::value_mask(mask, open),
        spans: envassign::value_spans(spans, open),
        quoted: fq,
    });
}

#[cfg(test)]
mod tests;
