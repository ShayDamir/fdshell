mod classify;
mod cmdline;
mod command;
mod for_block;
mod format;
pub(crate) mod if_block;
mod line;
mod pipeline;
mod semi;
mod token;
mod token_subst;
mod while_block;

pub use cmdline::{CommandLine, Pipeline};
pub use line::ParsedLine;

pub(crate) use format::format_parse_error;

use crate::error::parse::ParseErrorInfo;

pub fn parse(line: &[u8]) -> Result<ParsedLine, ParseErrorInfo> {
    let raw = token::tokenize(line)?;

    if let Some(pl) = line::detect(&raw)? {
        return Ok(pl);
    }

    if raw.first().is_some_and(|t| t.eq_bytes(b"if")) {
        return Ok(ParsedLine::If(if_block::tokens_to_if(&raw)?));
    }

    if raw.first().is_some_and(|t| t.eq_bytes(b"for")) {
        return Ok(ParsedLine::For(for_block::tokens_to_for(&raw)?));
    }

    if raw.first().is_some_and(|t| t.eq_bytes(b"while")) {
        return Ok(ParsedLine::While(while_block::tokens_to_loop(
            &raw, b"while",
        )?));
    }

    if raw.first().is_some_and(|t| t.eq_bytes(b"until")) {
        return Ok(ParsedLine::Until(while_block::tokens_to_loop(
            &raw, b"until",
        )?));
    }

    if raw.iter().any(|t| t.eq_bytes(b"|")) {
        return Ok(pipeline::parse_pipeline(&raw)?);
    }

    Ok(ParsedLine::Cmd(command::parse_command(&raw)?))
}

#[cfg(test)]
mod tests;
