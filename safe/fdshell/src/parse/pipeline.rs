use super::Token;
use crate::error::parse::ParseError;
use crate::parse::command::parse_command;
use crate::parse::heredoc::{HeredocBody, operator_count};
use crate::parse::{CommandLine, ParsedLine, Pipeline};
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::Position;

pub fn parse_pipeline(
    raw: &[Token],
    line: &[u8],
    specs: &[HeredocBody],
    set_at: Position,
) -> Result<ParsedLine, Report<ParseError>> {
    let mut commands = Vec::new();
    let mut start = 0;
    let mut spec_at = 0;
    for (i, (t, _, _, _, _)) in raw.iter().enumerate() {
        if t.eq_bytes(b"|") {
            ensure!(i != start, ParseError::UnexpectedPipe);
            let cmd = parse_stage(raw, line, specs, &mut spec_at, start, i, set_at)?;
            commands.push(cmd);
            start = i + 1;
        }
    }
    ensure!(start < raw.len(), ParseError::ExpectedCommandAfterPipe);
    let cmd = parse_stage(raw, line, specs, &mut spec_at, start, raw.len(), set_at)?;
    commands.push(cmd);
    Ok(ParsedLine::Pipeline(Pipeline { commands }))
}

/// Parse one pipeline stage: the tokens `[start..end)` trimmed at the first
/// `;` (beyond it lies heredoc body), with its share of the heredoc specs.
fn parse_stage(
    raw: &[Token],
    line: &[u8],
    specs: &[HeredocBody],
    spec_at: &mut usize,
    start: usize,
    end: usize,
    set_at: Position,
) -> Result<CommandLine, Report<ParseError>> {
    let stage = raw
        .get(start..end)
        .ok_or(ParseError::ExpectedCommandAfterPipe)?;
    let trimmed = trim_at_semi(stage);
    let count = operator_count(trimmed);
    let stage_specs = specs
        .get(*spec_at..*spec_at + count)
        .ok_or(ParseError::Never)?;
    *spec_at += count;
    parse_command(trimmed, line, stage_specs, set_at)
}

/// Drop the tokens after the first `;`: heredoc body words are not
/// command tokens.
fn trim_at_semi(tokens: &[Token]) -> &[Token] {
    let end = tokens
        .iter()
        .position(|(t, _, _, _, _)| t.eq_bytes(b";"))
        .unwrap_or(tokens.len());
    tokens.get(..end).unwrap_or(tokens)
}
