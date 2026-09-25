//! The token positions bash keeps literal during brace expansion: the
//! assignment word, case words and patterns, here-string words, and heredoc
//! delimiters (both the operator word and the terminating delimiter line).
//! Redirect words are *not* protected: bash brace-expands them (`> {a,b}` is
//! an "ambiguous redirect" in bash, a duplicate-redirect parse error here).

use crate::parse::{Token, delimiter_token_indices, literal_indices, word_indices};
use crate::scan::heredoc::body_regions;
use alloc::vec::Vec;

/// The indices of tokens that must not be brace-expanded.
pub(super) fn protected(line: &[u8], tokens: &[Token]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    if is_assignment(tokens) {
        out.push(0);
    }
    for i in literal_indices(tokens) {
        out.push(i);
    }
    for i in word_indices(tokens) {
        out.push(i);
    }
    for i in delimiter_token_indices(tokens) {
        out.push(i);
    }
    for i in heredoc_terminator_indices(line, tokens) {
        out.push(i);
    }
    out
}

/// The heredoc terminating delimiter lines: the body regions blank the body
/// but not the terminating line, so it survives tokenization as an ordinary
/// word. It is heredoc structure, never a command word, so it stays literal.
fn heredoc_terminator_indices(line: &[u8], tokens: &[Token]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for (_start, end) in body_regions(line) {
        if let Some(i) = tokens.iter().position(|(_, ts, _, _, _)| *ts == end) {
            out.push(i);
        }
    }
    out
}

/// The first token is an assignment word (`name=...` with a non-empty name);
/// bash applies no brace expansion to assignment words.
fn is_assignment(tokens: &[Token]) -> bool {
    tokens.first().is_some_and(|(t, _, _, _, _)| {
        t.split_once_byte(b'=')
            .is_some_and(|(lhs, _)| !lhs.is_empty())
    })
}
