mod array_ref;
mod backtick;
use alloc::vec::Vec;
mod bg_redirect;
mod builtin;
mod capture;
pub(crate) mod case_block;
pub(crate) mod case_clause;
mod classify;
mod cmdline;
mod command;
mod command_args;
mod comment;
mod detect;
mod detect_keyword;
mod dispatch;
mod elif;
mod emit;
mod fd_dup;
mod fd_path;
pub(crate) mod for_block;
pub(crate) mod function_block;
mod here_string;
mod heredoc;
pub(crate) mod if_block;
mod line;
mod pipeline;
mod quotes;
mod redirect;
mod redirect_op;
mod semi;
pub(crate) mod token;
mod token_pipe;
mod token_subst;
pub(crate) mod wait_block;
pub(crate) mod while_block;

pub(crate) use case_block::literal_indices;
pub use cmdline::{CommandLine, Pipeline};
pub(crate) use here_string::word_indices;
pub(crate) use heredoc::delimiter_token_indices;
pub use line::ParsedLine;

use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::ShortCStr;

/// A token: its unquoted word (empty for a quoted empty word such as `""`),
/// the byte range `(start, end)` of its raw text, whether it was fully
/// quoted, and the per-byte quote mask (parallel to the word; `true` marks
/// bytes that were inside double quotes and are protected from IFS word
/// splitting).
pub(crate) type Token = (ShortCStr, usize, usize, bool, Vec<bool>);

pub(crate) fn parse(text: &ScriptText) -> Result<ParsedLine, Report<ParseError>> {
    inner_parse(text)
}

fn inner_parse(text: &ScriptText) -> Result<ParsedLine, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let raw = token::tokenize_statement(line)?;

    if let Some(pl) = detect::detect(&raw)? {
        return Ok(pl);
    }

    if let Some(pl) = dispatch::dispatch_keyword(&raw, text)? {
        return Ok(pl);
    }

    let heredocs = heredoc::layout(line, &raw)?;

    if raw.iter().any(|(t, _, _, _, _)| t.eq_bytes(b"|")) {
        return pipeline::parse_pipeline(&raw, line, &heredocs, text.start);
    }

    Ok(ParsedLine::Cmd(command::parse_command(
        &raw, line, &heredocs, text.start,
    )?))
}

#[cfg(test)]
mod tests;
