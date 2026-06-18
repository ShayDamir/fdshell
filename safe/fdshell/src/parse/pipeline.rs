use crate::error::parse::ParseError;
use crate::parse::command::parse_command;
use crate::parse::{ParsedLine, Pipeline};
use sys::ShortCStr;

pub fn parse_pipeline(raw: &[ShortCStr]) -> Result<ParsedLine, ParseError> {
    let mut commands = Vec::new();
    let mut start = 0;
    for (i, t) in raw.iter().enumerate() {
        let bytes = t
            .as_bytes()
            .map_err(|_| ParseError::InvalidChar { ch: 0, pos: 0 })?;
        if bytes == b"|" {
            if i == start {
                return Err(ParseError::Reason {
                    pos: 0,
                    reason: "unexpected pipe",
                });
            }
            let slice = raw.get(start..i).ok_or(ParseError::Reason {
                pos: 0,
                reason: "expected command after pipe",
            })?;
            commands.push(parse_command(slice)?);
            start = i + 1;
        }
    }
    if start >= raw.len() {
        return Err(ParseError::Reason {
            pos: 0,
            reason: "expected command after pipe",
        });
    }
    let slice = raw.get(start..).ok_or(ParseError::Reason {
        pos: 0,
        reason: "expected command after pipe",
    })?;
    commands.push(parse_command(slice)?);
    Ok(ParsedLine::Pipeline(Pipeline { commands }))
}
