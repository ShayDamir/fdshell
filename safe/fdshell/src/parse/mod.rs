mod classify;
mod cmdline;
mod command;
mod for_block;
mod if_block;
mod line;
mod token;

pub use cmdline::{CommandLine, Pipeline};
pub use line::ParsedLine;

pub fn parse(line: &[u8]) -> Result<ParsedLine, i32> {
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

    if raw.iter().any(|t| t.eq_bytes(b"|")) {
        return command::parse_pipeline(&raw);
    }

    Ok(ParsedLine::Cmd(command::parse_command(&raw)?))
}

#[cfg(test)]
mod tests;
