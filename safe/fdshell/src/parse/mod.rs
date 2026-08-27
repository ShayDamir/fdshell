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

pub use cmdline::{CommandLine, Pipeline};
pub use line::ParsedLine;

use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::ShortCStr;

/// A token: its unquoted word, the byte range `(start, end)` of its raw text,
/// whether it was fully quoted, and the per-byte quote mask (parallel to the
/// word; `true` marks bytes that were inside double quotes and are protected
/// from IFS word splitting).
pub(crate) type Token = (ShortCStr, usize, usize, bool, Vec<bool>);

fn tokens_only(tokens: &[Token]) -> Vec<ShortCStr> {
    tokens.iter().map(|(t, _, _, _, _)| t.clone()).collect()
}

fn fully_quoted_only(tokens: &[Token]) -> Vec<bool> {
    tokens.iter().map(|(_, _, _, fq, _)| *fq).collect()
}

fn quote_masks_only(tokens: &[Token]) -> Vec<Vec<bool>> {
    tokens
        .iter()
        .map(|(_, _, _, _, mask)| mask.clone())
        .collect()
}

pub(crate) fn parse(text: &ScriptText) -> Result<ParsedLine, Report<ParseError>> {
    inner_parse(text)
}

fn inner_parse(text: &ScriptText) -> Result<ParsedLine, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let raw = token::tokenize(line)?;
    let tokens = tokens_only(&raw);

    if let Some(pl) = detect::detect(&raw)? {
        return Ok(pl);
    }

    if let Some(pl) = dispatch::dispatch_keyword(&raw, text)? {
        return Ok(pl);
    }

    if raw.iter().any(|(t, _, _, _, _)| t.eq_bytes(b"|")) {
        return pipeline::parse_pipeline(&raw, text.start);
    }

    let fully_quoted = fully_quoted_only(&raw);
    let quote_masks = quote_masks_only(&raw);
    Ok(ParsedLine::Cmd(command::parse_command(
        &tokens,
        fully_quoted,
        quote_masks,
        text.start,
    )?))
}

#[cfg(test)]
mod tests;
