use super::Token;
use crate::error::parse::ParseError;
use crate::parse::heredoc::HeredocBody;
use crate::parse::{ParsedLine, Pipeline, pipe_token_span};
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::Position;
use sys::ScriptText;

mod stage;

pub fn parse_pipeline(
    raw: &[Token],
    line: &[u8],
    specs: &[HeredocBody],
    set_at: Position,
    text: &ScriptText,
) -> Result<ParsedLine, Report<ParseError>> {
    let mut stages = Vec::new();
    let mut start = 0;
    let mut spec_at = 0;
    for (i, (_t, ts, te, _fq, _mask, _spans)) in raw.iter().enumerate() {
        if pipe_token_span(line, *ts, *te) {
            ensure!(i != start, ParseError::UnexpectedPipe);
            stages.push(stage::parse_stage(
                raw,
                line,
                specs,
                &mut spec_at,
                start..i,
                set_at,
                text,
            )?);
            start = i + 1;
        }
    }
    ensure!(start < raw.len(), ParseError::ExpectedCommandAfterPipe);
    stages.push(stage::parse_stage(
        raw,
        line,
        specs,
        &mut spec_at,
        start..raw.len(),
        set_at,
        text,
    )?);
    Ok(ParsedLine::Pipeline(Pipeline { stages }))
}
