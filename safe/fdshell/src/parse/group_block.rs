use super::Token;
use super::cond_bodies::claimed_regions;
use super::part_text::part_text;
use super::semi::block_body;
use super::semi::closing_keyword_index;
use super::semi::is_list_terminator;
use super::semi::token_range;
use super::value_spans;
use crate::error::parse::ParseError;
use crate::redirect::RedirectDef;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, ensure};
use sys::ScriptText;

mod tail;

/// The `{ …; }` group command (POSIX #1.2): the body text to run in the
/// current shell, and the redirections written after the closing brace.
#[cfg_attr(test, derive(Debug))]
pub struct GroupBlock {
    pub body: ScriptText,
    pub redirects: Vec<RedirectDef>,
}

/// Parse the group tokens: the body between the braces, the required list
/// terminator (`;`, newline or the plain `&`) before `}`, and the trailing
/// redirections.
pub(crate) fn tokens_to_group(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<GroupBlock, Report<ParseError>> {
    let opens_with_brace = tokens
        .first()
        .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"{"));
    ensure!(opens_with_brace, ParseError::MalformedGroupBlock);
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let close = closing_keyword_index(tokens, line).ok_or(ParseError::GroupMissingCloseBrace)?;
    let body_tokens = tokens.get(1..close).ok_or(ParseError::Never)?;
    // POSIX #1.2's `compound_list` must be non-empty, and the emptiness test is
    // [`block_body`]'s, shared with every block arm (the task #305 template).
    let trimmed = block_body(body_tokens, line, ParseError::GroupEmpty)?;
    // POSIX #1.2: `}` must be preceded by a command-list terminator. The
    // tokenizer emits a `;` token for a newline, so `{ echo a` newline `}` is
    // accepted, and the plain `&` backgrounds the body's last statement
    // (task #354: `{ echo hi & }`). `closing_keyword_index` skips token 0 (the
    // `{`), so `close` is never 0.
    let before_close = tokens.get(close.saturating_sub(1));
    ensure!(
        before_close.is_some_and(is_list_terminator),
        ParseError::ExpectedSemicolonBeforeCloseBrace
    );
    // The **extension** discriminator, at arm granularity (the pair
    // `parse/part_text` applies): a region that starts **inside** the group's own
    // text is part of the body slice's bytes, and the slice's last token is what
    // makes the verbatim slice cover it — trimming the bracketing `;` there would
    // cut the region out of the body text. Every other mode (no operator at all,
    // or **append**, where `part_text` re-attaches the region bytes behind the
    // slice) takes the trimmed slice, the `semi::trim_body` rule task #305 pins
    // for the definer body.
    let regions = claimed_regions(text, body_tokens);
    let extension = regions
        .first()
        .is_some_and(|&(start, _)| start < token_range(body_tokens).1);
    let body = part_text(text, if extension { body_tokens } else { trimmed })?;
    // A body region's words are blanked by the tokenizer (task #266) and a
    // terminator line's words are not, so an appended region leaves heredoc
    // **structure** in the tail; `tail::command_tail` cuts it off, and the tail
    // analysis sees only what the group's own text wrote after `}`.
    let tail_tokens = tail::command_tail(&split_close(tokens, close), &regions, !extension);
    let redirects = tail::redirects(&tail_tokens, line)?;
    Ok(GroupBlock { body, redirects })
}

/// The tokens after the closing brace, with `}` split off the glued form
/// (`}>out3` gives the `>out3` token, aligned with its raw range and quote
/// mask), so a glued redirect parses as the group's redirection.
fn split_close(tokens: &[Token], close: usize) -> Vec<Token> {
    let tail = tokens.get(close + 1..).unwrap_or(&[]).to_vec();
    let Some((t, start, end, fq, mask, spans)) = tokens.get(close).cloned() else {
        return tail;
    };
    if t.eq_bytes(b"}") {
        return tail;
    }
    let Some(rest) = t.get(1..) else {
        return tail;
    };
    let mut out = vec![(
        rest,
        start + 1,
        end,
        fq,
        mask.get(1..).unwrap_or(&[]).to_vec(),
        value_spans(&spans, 0),
    )];
    out.extend_from_slice(&tail);
    out
}

#[cfg(test)]
mod tests;
