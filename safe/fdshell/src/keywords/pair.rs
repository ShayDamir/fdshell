//! The glued cond-pair payload (task #236's S2): the keyword candidate the
//! token- and run-layer scorers must read from inside a glued word.

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
pub(crate) fn pair_payload(word: &[u8]) -> Option<&[u8]> {
    let at = word
        .windows(2)
        .position(|w| w == [b'|', b'|'] || w == [b'&', b'&'])?
        + 2;
    let rest = word.get(at..)?;
    let lead = rest.len() - rest.trim_ascii().len();
    let payload = rest.get(lead..)?;
    (!payload.is_empty() && !matches!(payload.first(), Some(b'|') | Some(b'&'))).then_some(payload)
}

#[cfg(test)]
mod tests;
