mod backtick;
use alloc::vec::Vec;
mod bg_redirect;
mod builtin;
mod capture;
pub(crate) mod case_block;
mod case_clause;
mod classify;
mod cmdline;
mod command;
mod comment;
mod detect;
mod detect_keyword;
mod elif;
mod emit;
pub(crate) mod for_block;
pub(crate) mod if_block;
mod line;
mod pipeline;
mod quotes;
mod redirect;
mod semi;
pub(crate) mod token;
mod token_pipe;
mod token_subst;
pub(crate) mod while_block;

pub use cmdline::{CommandLine, Pipeline};
pub use line::ParsedLine;

use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::ShortCStr;

/// A token: its unquoted word, the byte range `(start, end)` of its raw text,
/// and whether it was fully quoted.
pub(crate) type Token = (ShortCStr, usize, usize, bool);

fn tokens_only(tokens: &[Token]) -> Vec<ShortCStr> {
    tokens.iter().map(|(t, _, _, _)| t.clone()).collect()
}

fn fully_quoted_only(tokens: &[Token]) -> Vec<bool> {
    tokens.iter().map(|(_, _, _, fq)| *fq).collect()
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

    if raw.first().is_some_and(|(t, _, _, _)| t.eq_bytes(b"case")) {
        return Ok(ParsedLine::Case(case_block::tokens_to_case(&raw, text)?));
    }

    if raw.first().is_some_and(|(t, _, _, _)| t.eq_bytes(b"if")) {
        return Ok(ParsedLine::If(if_block::tokens_to_if(&raw, text)?));
    }

    if raw.first().is_some_and(|(t, _, _, _)| t.eq_bytes(b"for")) {
        return Ok(ParsedLine::For(for_block::tokens_to_for(&raw, text)?));
    }

    if raw.first().is_some_and(|(t, _, _, _)| t.eq_bytes(b"while")) {
        return Ok(ParsedLine::While(while_block::tokens_to_loop(
            &raw, b"while", text,
        )?));
    }
    if raw.first().is_some_and(|(t, _, _, _)| t.eq_bytes(b"until")) {
        return Ok(ParsedLine::Until(while_block::tokens_to_loop(
            &raw, b"until", text,
        )?));
    }

    if raw.iter().any(|(t, _, _, _)| t.eq_bytes(b"|")) {
        return pipeline::parse_pipeline(&raw, text.start);
    }

    let fully_quoted = fully_quoted_only(&raw);
    Ok(ParsedLine::Cmd(command::parse_command(
        &tokens,
        fully_quoted,
        text.start,
    )?))
}

#[cfg(test)]
mod tests;
