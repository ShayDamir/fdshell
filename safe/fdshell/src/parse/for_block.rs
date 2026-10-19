mod words;

use super::Token;
use crate::error::parse::ParseError;
use crate::parse::part_text::part_text;
use crate::parse::semi::{List, block_body, find_preceded_by_semi, up_to_closing};
use crate::parse::word_quoted::keyword_word;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, ensure};
use sys::ScriptText;
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug))]
pub struct ForBlock {
    pub var: ShortCStr,
    pub words: Vec<ShortCStr>,
    /// Per-byte quote masks, parallel to `words`.
    pub words_mask: Vec<Vec<bool>>,
    /// Zero-byte quote-span indices per word (task #321), parallel to
    /// `words_mask`.
    pub words_span: Vec<Vec<usize>>,
    /// Whether each word's raw span contained quotes, parallel to `words`
    /// (see `parse::word_quoted`).
    pub words_quoted: Vec<bool>,
    pub body: ScriptText,
}

pub(crate) fn tokens_to_for(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<ForBlock, Report<ParseError>> {
    ensure!(
        tokens
            .first()
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"for")),
        ParseError::ExpectedFor
    );
    let var = tokens
        .get(1)
        .ok_or(ParseError::ExpectedVariableName)?
        .0
        .clone();
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let in_pos = tokens
        .iter()
        .skip(2)
        .position(|tok| keyword_word(tok, line, b"in"))
        .ok_or(ParseError::ExpectedIn)?
        + 2;

    let do_idx =
        find_preceded_by_semi(tokens, in_pos, b"do", line).ok_or(ParseError::ExpectedDo)?;

    // A body-position heredoc's bodies sit in the block text after the closing
    // `done`: the tokens past it are body data, not block structure (task #273,
    // `semi::up_to_closing`). The `in` list is walked as `List::Words`, so a
    // closer-shaped word of the word list (`for x in done; do …; done`) is a
    // word, not the closer.
    let tokens = up_to_closing(tokens, line, List::Words, ParseError::ExpectedDone)?;
    // The closing `done` is the truncated slice's last token.
    let done_idx = tokens.len() - 1;
    ensure!(
        tokens
            .last()
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"done")),
        ParseError::ExpectedDone
    );
    ensure!(
        tokens
            .get(done_idx - 1)
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b";")),
        ParseError::ExpectedSemicolonBeforeDone
    );

    let body = part_text(
        text,
        block_body(
            tokens.get(do_idx + 1..done_idx).unwrap_or(&[]),
            ParseError::EmptyDoBody,
        )?,
    )?;

    let ws = words::collect_words(tokens, in_pos, do_idx)?;

    Ok(ForBlock {
        var,
        words: ws.words,
        words_mask: ws.words_mask,
        words_span: ws.words_span,
        words_quoted: ws.words_quoted,
        body,
    })
}
