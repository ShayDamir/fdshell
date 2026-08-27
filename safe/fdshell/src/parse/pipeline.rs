use super::Token;
use crate::error::parse::ParseError;
use crate::parse::command::parse_command;
use crate::parse::{ParsedLine, Pipeline};
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::Position;

pub fn parse_pipeline(raw: &[Token], set_at: Position) -> Result<ParsedLine, Report<ParseError>> {
    let mut commands = Vec::new();
    let mut start = 0;
    for (i, (t, _, _, _, _)) in raw.iter().enumerate() {
        if t.eq_bytes(b"|") {
            ensure!(i != start, ParseError::UnexpectedPipe);
            let cmd_tokens = raw
                .get(start..i)
                .ok_or(ParseError::ExpectedCommandAfterPipe)?;
            commands.push(parse_command(
                &super::tokens_only(cmd_tokens),
                super::fully_quoted_only(cmd_tokens),
                super::quote_masks_only(cmd_tokens),
                set_at,
            )?);
            start = i + 1;
        }
    }
    ensure!(start < raw.len(), ParseError::ExpectedCommandAfterPipe);
    let cmd_tokens = raw
        .get(start..)
        .ok_or(ParseError::ExpectedCommandAfterPipe)?;
    commands.push(parse_command(
        &super::tokens_only(cmd_tokens),
        super::fully_quoted_only(cmd_tokens),
        super::quote_masks_only(cmd_tokens),
        set_at,
    )?);
    Ok(ParsedLine::Pipeline(Pipeline { commands }))
}
