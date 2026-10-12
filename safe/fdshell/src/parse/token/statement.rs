//! Statement tokenization: the heredoc-aware entry point of the tokenizer.

use super::super::Token;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::Report;

/// Tokenize a statement whose heredoc bodies are opaque: the body spans are
/// blanked in a copy first, so arbitrary body bytes — including unbalanced
/// quotes — never break tokenization. Byte offsets are preserved.
pub(crate) fn tokenize_statement(line: &[u8]) -> Result<Vec<Token>, Report<ParseError>> {
    let regions = crate::scan::heredoc::body_regions(line);
    if regions.is_empty() {
        return super::tokenize(line);
    }
    let mut masked: Vec<u8> = line.to_vec();
    for (start, end) in regions {
        if let Some(body) = masked.get_mut(start..end) {
            for b in body.iter_mut() {
                *b = b' ';
            }
        }
    }
    super::tokenize(&masked)
}
