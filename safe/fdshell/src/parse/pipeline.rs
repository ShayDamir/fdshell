use crate::error::parse::{ParseError, report_error};
use crate::parse::command::parse_command;
use crate::parse::{ParsedLine, Pipeline};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub fn parse_pipeline(raw: &[(ShortCStr, usize, bool)]) -> Result<ParsedLine, Report<ParseError>> {
    let mut commands = Vec::new();
    let mut start = 0;
    for (i, (t, _, _)) in raw.iter().enumerate() {
        let bytes = t.as_bytes().change_context(ParseError::Reason {
            reason: "internal string state",
        })?;
        if bytes == b"|" {
            if i == start {
                return Err(report_error("unexpected pipe", 0));
            }
            let cmd_tokens = raw
                .get(start..i)
                .ok_or_else(|| report_error("expected command after pipe", 0))?;
            let fq: Vec<bool> = cmd_tokens.iter().map(|(_, _, fq)| *fq).collect();
            let tokens: Vec<ShortCStr> = cmd_tokens.iter().map(|(t, _, _)| t.clone()).collect();
            commands.push(parse_command(&tokens, fq)?);
            start = i + 1;
        }
    }
    if start >= raw.len() {
        return Err(report_error("expected command after pipe", 0));
    }
    let cmd_tokens = raw
        .get(start..)
        .ok_or_else(|| report_error("expected command after pipe", 0))?;
    let fq: Vec<bool> = cmd_tokens.iter().map(|(_, _, fq)| *fq).collect();
    let tokens: Vec<ShortCStr> = cmd_tokens.iter().map(|(t, _, _)| t.clone()).collect();
    commands.push(parse_command(&tokens, fq)?);
    Ok(ParsedLine::Pipeline(Pipeline { commands }))
}
