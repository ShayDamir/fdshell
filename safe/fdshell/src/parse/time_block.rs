use super::Token;
use super::semi::verbatim;
use super::word_quoted::word_quoted;
use crate::error::parse::ParseError;
use crate::keywords::keyword_delta;
use crate::parse::line::ParsedLine;
use alloc::boxed::Box;
use error_stack::{Report, ResultExt};
use sys::ScriptText;

/// A `time` prefix statement (POSIX XCU 2.4 + the `time` utility page): the
/// report layout flag and the statement to time.
///
/// All leading `time [-p]` prefixes are collapsed at parse, so `inner` never
/// starts with another `time` and one statement prints exactly one report.
pub struct TimeBlock {
    /// `-p` was seen: report in the POSIX layout instead of bash's.
    pub posix: bool,
    /// The statement to time; `None` for a bare `time` (report zeros, status 0).
    pub inner: Option<Box<ParsedLine>>,
    /// The inner statement's text: a verbatim span of the enclosing statement,
    /// so positions and heredoc bodies stay inside it.
    pub inner_text: ScriptText,
}

/// The `time` prefix: recognized when the statement's first word is `time`
/// with none of its characters quoted (XCU 2.4), i.e. the reserved word, not
/// the `time` utility. Every prefix is stripped in a loop, so the remainder is
/// parsed once — no recursion over the prefix count.
pub(super) fn detect(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let is_prefix = tokens
        .first()
        .is_some_and(|(w, s, e, _, _, _)| w.eq_bytes(b"time") && !word_quoted(w, *s, *e));
    if !is_prefix {
        return Ok(None);
    }
    let mut posix = false;
    // Tokens consumed as prefixes: every leading `time` and the `-p` written
    // directly behind it — the same run `keywords::time_prefix_len` measures, so
    // the parse hook and the segment scanner strip the same bytes. Collapsing the
    // run in this loop keeps one parse pass and one report for `time time -p …`
    // (no recursion over the prefix count). A prefix run ends at the first word
    // that is not a `time` with its optional flag: a `-p` that follows a statement
    // is one of its words (`time -p -p ls` runs the command `-p`, as bash does),
    // and so is a quoted `-p` (`time "-p" ls`), so the flag is taken only from an
    // unquoted `-p` right behind a `time`.
    let mut taken = 0usize;
    while let Some((w, s, e, _, _, _)) = tokens.get(taken) {
        if !w.eq_bytes(b"time") || word_quoted(w, *s, *e) {
            break;
        }
        taken += 1;
        if let Some((w, s, e, _, _, _)) = tokens.get(taken)
            && w.eq_bytes(b"-p")
            && !word_quoted(w, *s, *e)
        {
            posix = true;
            taken += 1;
        }
    }
    let rest = tokens.get(taken..).unwrap_or(&[]);
    // A bare `time` has no inner statement: it reports zeros and succeeds.
    let Some(inner_text) = inner_text(rest, text)? else {
        return Ok(Some(ParsedLine::Time(TimeBlock {
            posix,
            inner: None,
            inner_text: text.subslice(0, 0).ok_or(ParseError::Never)?,
        })));
    };
    let inner = Some(Box::new(super::parse(&inner_text)?));
    Ok(Some(ParsedLine::Time(TimeBlock {
        posix,
        inner,
        inner_text,
    })))
}

/// The timed statement's text: `None` when there is nothing to time.
///
/// A block opener owns everything the segment holds after it (`while … done`,
/// `{ …; } > out`), so the inner text is the segment text minus the `time`
/// prefix: the tokens stop at the first separator, so their span cannot reach
/// the block's closing keyword. Any other statement (a command, a pipeline) is
/// a verbatim span of its tokens, which keeps its positions and heredoc bodies.
fn inner_text(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<Option<ScriptText>, Report<ParseError>> {
    let Some((word, start, end, _, _, _)) = tokens.first() else {
        return Ok(None);
    };
    let line = text.as_bytes().change_context(ParseError::Never)?;
    // `keyword_delta` is a sign projection with no `0` row, so the rule is the
    // predicate `is_positive` and not a `> 0` comparison: a comparison exposes
    // a `>=`/`==`/`<` family that accepts exactly the same values. A `0` arm in
    // the scorer would make such a comparison live again;
    // `keywords/tests.rs::keyword_delta_wait_block_vs_builtin` is the row that
    // notices the change.
    if !word_quoted(word, *start, *end)
        && keyword_delta(
            word.as_bytes().change_context(ParseError::Never)?,
            line,
            *end,
        )
        .is_some_and(i32::is_positive)
    {
        return Ok(text.subslice(*start, text.data.len().saturating_sub(*start)));
    }
    Ok(Some(verbatim(text, tokens)?))
}

#[cfg(test)]
mod tests;
