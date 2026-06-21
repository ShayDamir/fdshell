use crate::error::parse::ParseError;
use crate::error::parse::report_error;
use crate::parse::command::parse_command;
use crate::parse::{ParsedLine, Pipeline};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub fn parse_pipeline(raw: &[ShortCStr]) -> Result<ParsedLine, Report<ParseError>> {
    let mut commands = Vec::new();
    let mut start = 0;
    for (i, t) in raw.iter().enumerate() {
        let bytes = t.as_bytes().change_context(ParseError::Reason {
            reason: "internal string state",
        })?;
        if bytes == b"|" {
            if i == start {
                return Err(report_error("unexpected pipe", 0));
            }
            let slice = raw
                .get(start..i)
                .ok_or_else(|| report_error("expected command after pipe", 0))?;
            commands.push(parse_command(slice)?);
            start = i + 1;
        }
    }
    if start >= raw.len() {
        return Err(report_error("expected command after pipe", 0));
    }
    let slice = raw
        .get(start..)
        .ok_or_else(|| report_error("expected command after pipe", 0))?;
    commands.push(parse_command(slice)?);
    Ok(ParsedLine::Pipeline(Pipeline { commands }))
}
