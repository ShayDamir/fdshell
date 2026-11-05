//! The glued cond-pair payload (task #236's S2): the keyword candidate the
//! token- and run-layer scorers must read from inside a glued word.

use crate::mask::{MaskBit, quoted_at};

/// The *pair payload* of a word: the keyword candidate behind a glued cond
/// *pair* in the word's interior — `a||if` → `if` after skipping whitespace
/// (`a && fi` → `fi`). `None` when the word carries no pair (a pipeline `|`
/// is a single byte, never a pair — the word behind it is scored by the
/// stage arm, task #292) or the payload is empty/operator-shaped. Task
/// #236's S2: the pair opens *two* word starts
/// for the byte-level scans (their [`Glue`] payload rule); a token- or
/// run-layer scorer that sees the glued word whole must score the payload
/// too, or the keyword glued behind the pair never nests (`do echo a||if :;
/// …; fi` closes the enclosing block at the `fi`). The pair's *left* operand
/// keeps the word's own place rule; the payload's own end offset belongs to
/// the caller, which knows the word's end in `line`.
///
/// `mask` is the word's per-byte quote mask and gates the **pair's own bytes**
/// (POSIX §2.9.1, task #530 / bug #474): in `a"&&"if` the `&&` is quoted, so the
/// word is one argument and carries no pair at all — bash reads the whole word as
/// `echo`'s word. A byte-level caller passes an empty mask (padding answers
/// *unquoted*) and keeps the text-only rule; there the word never looks like this,
/// because the byte splitters break words on raw operator bytes and a quoted
/// operator keeps its quote bytes.
pub(crate) fn pair_payload<'a>(word: &'a [u8], mask: &[MaskBit]) -> Option<&'a [u8]> {
    let op = word
        .windows(2)
        .position(|w| w == [b'|', b'|'] || w == [b'&', b'&'])?;
    if quoted_at(mask, op) {
        return None;
    }
    let rest = word.get(op + 2..)?;
    let lead = rest.len() - rest.trim_ascii().len();
    let payload = rest.get(lead..)?;
    (!payload.is_empty() && !matches!(payload.first(), Some(b'|') | Some(b'&'))).then_some(payload)
}

#[cfg(test)]
mod tests;
