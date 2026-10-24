//! The statement parse chain: from one line's bytes to a `ParsedLine`.
//!
//! The order of the five detection arms is the contract, not an implementation
//! detail: `((…))` must be recognized on the raw bytes *before* tokenizing
//! (LESSONS.md `#351`), and `tokenize_statement`'s blanking contract (LESSONS.md
//! `#311`) is what the keyword, `time` and heredoc arms then rely on.

use crate::error::parse::ParseError;
use crate::parse::{ParsedLine, Token, pipe_token_span};
use crate::parse::{arith_cmd, command, detect, dispatch, heredoc, pipeline, time_block, token};
use error_stack::Report;
use sys::ScriptText;

/// Parse one statement: the arithmetic-command, keyword, `time` and heredoc
/// arms in order, then a pipeline or a single command.
pub(crate) fn parse_statement(
    text: &ScriptText,
    line: &[u8],
) -> Result<ParsedLine, Report<ParseError>> {
    if let Some(parsed) = arith_cmd::detect(text, line)? {
        return Ok(parsed);
    }
    let raw = token::tokenize_statement(line)?;

    if let Some(pl) = detect::detect(&raw)? {
        return Ok(pl);
    }

    if let Some(pl) = dispatch::dispatch_keyword(&raw, text)? {
        return Ok(pl);
    }

    if let Some(pl) = time_block::detect(&raw, text)? {
        return Ok(pl);
    }

    let heredocs = heredoc::layout(line, &raw)?;

    if has_pipe(&raw, line) {
        return pipeline::parse_pipeline(&raw, line, &heredocs, text.start, text);
    }

    Ok(ParsedLine::Cmd(command::parse_command(
        &raw, line, &heredocs, text.start,
    )?))
}

/// True when a raw token covers a pipe byte, i.e. the statement is a pipeline.
fn has_pipe(raw: &[Token], line: &[u8]) -> bool {
    raw.iter()
        .any(|(_t, start, end, _fq, _mask, _spans)| pipe_token_span(line, *start, *end))
}
