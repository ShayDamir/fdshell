//! The group's **tail**: the redirections written after the closing brace.
//! Extracted from `parse/group_block.rs` for the line budget (task #354); the
//! closer logic (`split_close`) stays with the closer rule.

use super::Token;
use crate::error::parse::ParseError;
use crate::parse::classify::parse_redirect;
use crate::parse::word_quoted::span_word;
use crate::redirect::RedirectDef;
use crate::scan::heredoc::span_operator;
use alloc::vec::Vec;
use error_stack::{Report, bail};

/// Cut the here-doc structure the builder appended **behind** the closing brace
/// out of the tail. In **append** mode the group's own regions travel after `}`
/// (`parse/part_text`, the rule every block arm shares, task #315): a **body**
/// region's words are blanked by the tokenizer (task #266) and a **terminator**
/// line's words are not, so the delimiter word and the newlines around the
/// region stay tokens. The cut is the newline the append inserts **before** the
/// first region: every token at/after it is heredoc structure, never something
/// the group's own text wrote after `}` (a `} > out` word starts before that
/// newline, so it stays). With no append — no region, or extension mode, where
/// the regions are interleaved with the group's own lines — the tail is exactly
/// the group's text and nothing is cut, which keeps every pinned tail message
/// byte-ident.
pub(super) fn command_tail(tail: &[Token], regions: &[(usize, usize)], append: bool) -> Vec<Token> {
    let Some(&(first, _)) = regions.first().filter(|_| append) else {
        return tail.to_vec();
    };
    // The appended newline is the byte just before the region's first body line.
    let cut = first.saturating_sub(1);
    tail.iter()
        .filter(|(_, start, _, _, _, _)| *start < cut)
        .cloned()
        .collect()
}

/// The redirections written after `}`, in source order. A here-doc/here-string
/// operator is checked first (it starts with `<`, which the read redirect would
/// otherwise swallow); a pipeline, background, here-doc, or here-string form on
/// the group is a parse error (each is a separate task), and so is a trailing
/// plain word. The here-doc test is the shared raw-span shape rule, so the
/// io-number form `{ …; } 3<<T` keeps the here-doc message instead of degrading
/// to a trailing word.
pub(super) fn redirects(
    tail: &[Token],
    line: &[u8],
) -> Result<Vec<RedirectDef>, Report<ParseError>> {
    let mut redirects = Vec::new();
    let mut i = 0;
    while i < tail.len() {
        let tok @ (t, start, end, _fq, _mask, _spans) = tail.get(i).ok_or(ParseError::Never)?;
        if t.starts_with(b"<<") || span_operator(line, *start, *end).is_some() {
            bail!(ParseError::GroupHereDocNotSupported);
        }
        if let Some((def, extra)) = parse_redirect(tail, i)? {
            redirects.push(def);
            i += extra + 1;
            continue;
        }
        // The whole-word `|` test is [`span_word`]'s, so a quoted `"|"` falls
        // through to the trailing-word row it belongs to (task #336).
        if span_word(tok, b"|") {
            bail!(ParseError::UnexpectedPipe);
        }
        if t.starts_with(b"&") {
            bail!(ParseError::UnexpectedChar { ch: b'&' });
        }
        bail!(ParseError::GroupTrailingWord);
    }
    Ok(redirects)
}

#[cfg(test)]
mod tests;
