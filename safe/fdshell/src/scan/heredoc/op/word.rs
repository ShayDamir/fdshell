//! The operator word as **one unit** for the byte scanners: the `<<` word plus
//! its operand word — glued (`<<}`, `<<-}`, `3<<}`) or bare (`<< }`, `<< {`) —
//! so no byte inside either half moves a brace depth (task #432). The region
//! half of the same rule (the body and its terminator line) is
//! `scan::heredoc::{body_regions, terminator_spans}`; the operand walk is
//! [`word_span`]'s, the walk the delimiter word is read with (quotes, escape
//! pairs and `$( … )` regions consumed whole).

use crate::bytes::{QUOTE, SQUOTE};
use crate::scan::heredoc::lines::{token_word_end, word_span};
use crate::scan::heredoc::{attached, operator_delim, span_operator};

/// The index just past the here-doc operator word that starts at `i` **and its
/// operand word**; `None` when `i` opens no here-doc operator word, when a bare
/// `<<` has no operand word, and when the operand is **quoted** — a quoted word
/// is word data, so the caller's own quote tracking keeps owning it (LESSONS
/// "A heredoc terminator line is structure, not command bytes": the words of a
/// structure line stay words, and that is what `brace_expand/protect` protects).
pub(crate) fn word_with_operand_end(line: &[u8], i: usize) -> Option<usize> {
    let op_word_end = word_end(line, i)?;
    let (op, _prefix) = span_operator(line, i, op_word_end)?;
    let (strip, _) = operator_delim(line, op, op_word_end);
    // The operand the operator word carries sits past `<<` and its one `-`
    // marker; a bare `<<`'s operand is the next word across spaces and tabs.
    let operand = if attached(line, op, op_word_end) {
        op + 2 + usize::from(strip)
    } else {
        word_start_after_space(line, op_word_end)
    };
    let end = word_end(line, operand)?;
    if end == operand || is_quoted(line.get(operand..end)?) {
        return None;
    }
    Some(end)
}

/// The end of the word starting at `i` — [`word_span`]'s walk over the
/// tokenizer's word-break set, the same one `lines::delimiter_span` uses.
fn word_end(line: &[u8], mut i: usize) -> Option<usize> {
    let start = i;
    while i < line.len() && !token_word_end(line.get(i).copied().unwrap_or(0)) {
        i = word_span(line, i, line.len());
    }
    (i > start).then_some(i)
}

/// The first word start after `i`, across spaces and tabs — the bare `<<`
/// form's operand word.
fn word_start_after_space(line: &[u8], mut i: usize) -> usize {
    loop {
        if (i..line.len()).is_empty() {
            break;
        }
        if !matches!(line.get(i).copied(), Some(b' ') | Some(b'\t')) {
            break;
        }
        i += 1;
    }
    i
}

/// A **quoted** operand is word data: every byte scanner consumes a quoted span
/// as one region of its own (LESSONS "A quoted region is a unit for *every*
/// byte scanner"), so the helper must not read a quoted brace as structure.
fn is_quoted(word: &[u8]) -> bool {
    word.iter().any(|&b| b == QUOTE || b == SQUOTE)
}

#[cfg(test)]
mod tests;
