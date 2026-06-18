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

/// Extract just the `ShortCStr` values from position-tagged tokens.
fn tokens_only(tokens: &[(sys::ShortCStr, usize)]) -> Vec<sys::ShortCStr> {
    tokens.iter().map(|(t, _)| t.clone()).collect()
}

pub fn parse(line: &[u8]) -> Result<ParsedLine, crate::error::parse::ParseErrorInfo> {
    let raw = token::tokenize(line)?;
    let tokens = tokens_only(&raw);

    if let Some(pl) = line::detect(&tokens)? {
        return Ok(pl);
    }

    if raw.first().is_some_and(|(t, _)| t.eq_bytes(b"if")) {
        return Ok(ParsedLine::If(if_block::tokens_to_if(&raw)?));
    }

    if raw.first().is_some_and(|(t, _)| t.eq_bytes(b"for")) {
        return Ok(ParsedLine::For(for_block::tokens_to_for(&raw)?));
    }

    if raw.first().is_some_and(|(t, _)| t.eq_bytes(b"while")) {
        return Ok(ParsedLine::While(while_block::tokens_to_loop(
            &raw, b"while",
        )?));
    }

    if raw.first().is_some_and(|(t, _)| t.eq_bytes(b"until")) {
        return Ok(ParsedLine::Until(while_block::tokens_to_loop(
            &raw, b"until",
        )?));
    }

    if raw.iter().any(|(t, _)| t.eq_bytes(b"|")) {
        return Ok(pipeline::parse_pipeline(&tokens)?);
    }

    Ok(ParsedLine::Cmd(command::parse_command(&tokens)?))
}

#[cfg(test)]
mod tests;
