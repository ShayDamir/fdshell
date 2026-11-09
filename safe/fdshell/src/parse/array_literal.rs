// mutants: -C --lib -- -E 'test(/^parse::array_literal::/)'

//! The array-assignment word `name=(w1 w2 …)` (task #422): the region assembly
//! at the **token** level, and the two types the runtime reads.
//!
//! The tokenizer keeps `m=(1` as one word and emits the `)` as its own separator
//! token, so the region is assembled here, where the whole statement's tokens
//! already are — exactly where `envassign::collect_all` builds its statements.
//! Nothing in the tokenizer or the `case` clause scan moves: the byte/token
//! agreement of LESSONS ("the byte- and token-level scans must agree") is what
//! task #300 pinned, and this module only reads the tokens it is handed.

mod words;

use alloc::vec::Vec;

use crate::error::parse::ParseError;
use crate::mask::MaskBit;
use crate::parse::Token;
use crate::parse::word_quoted::span_word;
use error_stack::{Report, bail};
use sys::ShortCStr;

use words::{all_array, open_word, push};

/// One word of an array literal, kept exactly as the tokenizer produced it: text,
/// mask, spans and the fully-quoted flag travel together (the parallel plumbing
/// of `parse.rs`), because the runtime expands every element word through the
/// ordinary per-word pipeline.
#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub(crate) struct ArrayWord {
    pub(crate) text: ShortCStr,
    pub(crate) mask: Vec<MaskBit>,
    pub(crate) spans: Vec<usize>,
    pub(crate) quoted: bool,
}

/// One `name=(…)` word: the name as written (`+=` marked, the sign not part of
/// it) and the region's words, in the order they were written.
#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub(crate) struct ArrayAssign {
    pub(crate) name: ShortCStr,
    pub(crate) append: bool,
    pub(crate) elements: Vec<ArrayWord>,
}

/// The `parse::detect` arm, before `envassign`'s walk: `Some` when the whole
/// statement (up to the first `;`) is array-assignment words, `None` as soon as
/// a word is not one — a mixed statement is `envassign`'s, and so is a scoped
/// prefix (`a=(1 2) echo hi`, pinned as measured).
pub(crate) fn detect(tokens: &[Token]) -> Result<Option<Vec<ArrayAssign>>, Report<ParseError>> {
    let mut assigns: Vec<ArrayAssign> = Vec::new();
    let mut open: Option<ArrayAssign> = None;
    for tok @ Token {
        word: text,
        fully_quoted: fq,
        mask,
        spans,
        ..
    } in tokens
    {
        if span_word(tok, b";") {
            break;
        }
        if let Some(assign) = open.as_mut() {
            // Inside a region every word is an element until the tokenizer's own
            // `)` token closes it; a quoted or escaped `)` is a word, never a
            // region end (the agreement task #300 pinned).
            if text.eq_bytes(b")") {
                assigns.push(open.take().ok_or(ParseError::Never)?);
            } else {
                push(assign, text, mask, spans, *fq);
            }
            continue;
        }
        let Some(opened) = open_word(tok) else {
            // A word that is no array assignment ends the arm's claim: the
            // statement is the walk `detect` is called from's, exactly as it was.
            return Ok(None);
        };
        open = Some(opened);
    }
    if open.is_some() {
        // The region is bounded by the statement, so an unclosed one is this
        // arm's own report (bash rejects the shape at rc 2, task #422).
        bail!(ParseError::ArrayLiteralMissingCloseParen);
    }
    Ok(all_array(&assigns))
}

#[cfg(test)]
mod tests;
