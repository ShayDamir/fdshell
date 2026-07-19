use crate::error::parse::ParseError;
use crate::parse::command::parse_command;
use crate::parse::{ParsedLine, Pipeline};
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::ShortCStr;

pub fn parse_pipeline(raw: &[(ShortCStr, usize, bool)]) -> Result<ParsedLine, Report<ParseError>> {
    let mut commands = Vec::new();
    let mut start = 0;
    for (i, (t, _, _)) in raw.iter().enumerate() {
        if t.eq_bytes(b"|") {
            ensure!(i != start, ParseError::UnexpectedPipe);
            let cmd_tokens = raw
                .get(start..i)
                .ok_or(ParseError::ExpectedCommandAfterPipe)?;
            let fq: Vec<bool> = cmd_tokens.iter().map(|(_, _, fq)| *fq).collect();
            let tokens: Vec<ShortCStr> = cmd_tokens.iter().map(|(t, _, _)| t.clone()).collect();
            commands.push(parse_command(&tokens, fq)?);
            start = i + 1;
        }
    }
    ensure!(start < raw.len(), ParseError::ExpectedCommandAfterPipe);
    let cmd_tokens = raw
        .get(start..)
        .ok_or(ParseError::ExpectedCommandAfterPipe)?;
    let fq: Vec<bool> = cmd_tokens.iter().map(|(_, _, fq)| *fq).collect();
    let tokens: Vec<ShortCStr> = cmd_tokens.iter().map(|(t, _, _)| t.clone()).collect();
    commands.push(parse_command(&tokens, fq)?);
    Ok(ParsedLine::Pipeline(Pipeline { commands }))
}
