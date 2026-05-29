mod classify;
mod cmdline;
mod command;
mod line;
mod token;

pub use cmdline::{CommandLine, Pipeline};
pub use line::ParsedLine;

pub fn parse(line: &str) -> Result<ParsedLine, i32> {
    let raw = token::tokenize(line)?;

    if let Some(pl) = line::detect(&raw)? {
        return Ok(pl);
    }

    if raw.iter().any(|t| t.as_bytes() == b"|") {
        return command::parse_pipeline(&raw);
    }

    Ok(ParsedLine::Cmd(command::parse_command(&raw)?))
}

#[cfg(test)]
mod tests;
