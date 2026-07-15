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
mod token;
mod token_subst;
mod while_block;

pub use cmdline::{CommandLine, Pipeline};
pub use line::ParsedLine;

use crate::error::parse::ParseError;
use error_stack::Report;

fn tokens_only(tokens: &[(sys::ShortCStr, usize, bool)]) -> Vec<sys::ShortCStr> {
    tokens.iter().map(|(t, _, _)| t.clone()).collect()
}

fn fully_quoted_only(tokens: &[(sys::ShortCStr, usize, bool)]) -> Vec<bool> {
    tokens.iter().map(|(_, _, fq)| *fq).collect()
}

pub(crate) fn parse(line: &[u8]) -> Result<ParsedLine, Report<ParseError>> {
    inner_parse(line)
}

fn inner_parse(line: &[u8]) -> Result<ParsedLine, Report<ParseError>> {
    let raw = token::tokenize(line)?;
    let tokens = tokens_only(&raw);

    if let Some(pl) = detect::detect(&raw)? {
        return Ok(pl);
    }

    if raw.first().is_some_and(|(t, _, _)| t.eq_bytes(b"case")) {
        return Ok(ParsedLine::Case(case_block::tokens_to_case(&raw)?));
    }

    if raw.first().is_some_and(|(t, _, _)| t.eq_bytes(b"if")) {
        return Ok(ParsedLine::If(if_block::tokens_to_if(&raw)?));
    }

    if raw.first().is_some_and(|(t, _, _)| t.eq_bytes(b"for")) {
        return Ok(ParsedLine::For(for_block::tokens_to_for(&raw)?));
    }

    if raw.first().is_some_and(|(t, _, _)| t.eq_bytes(b"while")) {
        return Ok(ParsedLine::While(while_block::tokens_to_loop(
            &raw, b"while",
        )?));
    }
    if raw.first().is_some_and(|(t, _, _)| t.eq_bytes(b"until")) {
        return Ok(ParsedLine::Until(while_block::tokens_to_loop(
            &raw, b"until",
        )?));
    }

    if raw.iter().any(|(t, _, _)| t.eq_bytes(b"|")) {
        return pipeline::parse_pipeline(&raw);
    }

    let fully_quoted = fully_quoted_only(&raw);
    Ok(ParsedLine::Cmd(command::parse_command(
        &tokens,
        fully_quoted,
    )?))
}

#[cfg(test)]
mod tests;
