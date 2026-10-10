//! The byte-level `<<` operator scan for a run, shared by the skip helpers
//! and the line-body extraction.

use crate::scan::ScanState;

use super::lines::delimiter_span;
use super::op::Operator;
use alloc::vec::Vec;

/// The `Operator` of every `<<` in the run, in order. `None` when a bare
/// `<<` (or `<<-`) has no delimiter word. A `#` comment (outside quotes, at
/// a word start) is skipped whole, so a `<<` in a comment is not an operator
/// (the tokenizer never yields it, and the two counts must agree).
pub(crate) fn operator_delims(line: &[u8], from: usize, to: usize) -> Option<Vec<Operator>> {
    let mut state = ScanState::new();
    let mut found: Vec<Operator> = Vec::new();
    let mut seen_word = false;
    let mut i = from;
    while i < to {
        let bare = !state.in_quote && !state.in_backtick && state.paren_depth == 0;
        if bare && !state.word_active && line.get(i) == Some(&b'#') {
            match line
                .get(i..)
                .and_then(|s| s.iter().position(|&b| b == b'\n'))
            {
                Some(p) => i = i + p + 1,
                None => return Some(found),
            }
            continue;
        }
        if bare
            && seen_word
            && word_start(line, i, &state)
            && line.get(i) == Some(&b'<')
            && line.get(i + 1) == Some(&b'<')
            && line.get(i + 2) != Some(&b'<')
            && !precedes_pipe(line, i)
        {
            let dash = line.get(i + 2) == Some(&b'-');
            let (next, raw) = delimiter_span(line, i + 2 + usize::from(dash), to)?;
            found.push(Operator::new(raw, dash));
            while i < next {
                i = state.advance(line, i);
            }
            seen_word = true;
            continue;
        }
        i = state.advance(line, i);
        if state.word_active {
            seen_word = true;
        }
    }
    Some(found)
}

/// `i` starts a token word: the run start or a tokenizer word-break byte.
/// A `)` breaks the word only at top level — a `$( )`-closing `)` keeps the
/// token whole, exactly as the tokenizer does.
fn word_start(line: &[u8], i: usize, state: &ScanState) -> bool {
    if i == 0 {
        return true;
    }
    match line.get(i - 1).copied() {
        Some(b')') => !state.word_active,
        // The `_` arm excludes `|`: a `|` byte never starts a word here, so a
        // `<<` right after `|` (absorbed into a `>|` operator or a pipeline
        // pipe) is never an operator; `precedes_pipe` decides the pipe.
        _ => matches!(
            line.get(i - 1).copied(),
            Some(b' ') | Some(b'\t') | Some(b';') | Some(b'\n')
        ),
    }
}

/// A `|` with only whitespace between it and `i`: a pipeline position, where
/// `<<` is a command word, not an operator. A `|` absorbed into a `>|` clobber
/// operator is not a pipeline position: `parse::redirect::clobber_pipe` is the
/// ONE rule, the same `>`-terminated word test the tokenizer applies to the
/// word's raw bytes + mask (`parse/token_pipe.rs`), so the byte and token
/// counts cannot diverge (LESSONS: byte-level and token-level rules must
/// agree).
fn precedes_pipe(line: &[u8], i: usize) -> bool {
    let mut k = i;
    while k > 0 {
        k -= 1;
        match line.get(k) {
            Some(&b' ') | Some(b'\t') => continue,
            Some(b'|') => return !crate::parse::clobber_pipe(line, k),
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests;
