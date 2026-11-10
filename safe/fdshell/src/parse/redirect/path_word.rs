// mutants: -C --lib -C --test -C redirect -- -E 'test(/^parse::/) | binary(redirect)'

//! The redirect target **word**: the one derivation of its text, quote mask,
//! zero-byte quote spans and `quoted` flag, so the bare operator form (the
//! operand is a whole token) and the attached form (the word is a token
//! suffix) hand the opener the same quadruple an argument word carries
//! (task #278).

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::here_string::{word_mask, word_spans};
use crate::redirect::RedirectSource;
use error_stack::{Report, bail};

/// The whole operand token: `> word`, `< word`, the bare `&> word`. The token
/// *is* the target word, so text, mask and spans are taken as they come from
/// the tokenizer.
impl Token {
    pub(crate) fn full(&self) -> RedirectSource {
        RedirectSource::path_word(
            self.word.clone(),
            self.mask.clone(),
            self.spans.clone(),
            self.word.is_shorter_than_span(self.start, self.end),
        )
    }
}

/// The target inside the operator token (`>f`, `>f$(echo x)`, `&>f`): the word
/// is the token's suffix from `offset`, the index one past the operator's last
/// byte. The mask and the zero-byte spans are sliced at that same offset and
/// the spans shift down to word indices — the same pair of owners the
/// here-string word uses, so the word and its quoting cannot drift (LESSONS).
/// `quoted` is the whole token's flag: the suffix's own raw span is not
/// representable, and over-true is safe — the flag only keeps one empty word
/// when the expansion is empty.
pub(crate) fn suffix(token: &Token, offset: usize) -> Result<RedirectSource, Report<ParseError>> {
    let Token {
        word: t,
        start,
        end,
        mask,
        spans,
        ..
    } = token;
    let Some(path) = t.get(offset..) else {
        bail!(ParseError::Never);
    };
    Ok(RedirectSource::path_word(
        path,
        word_mask(mask, offset),
        word_spans(spans, offset),
        t.is_shorter_than_span(*start, *end),
    ))
}
