//! One pipeline stage. A stage is a full **command**, so its first word goes
//! through the same keyword dispatch the statement path uses
//! (`parse::inner_parse`): `echo a | while :; do echo b; break; done` runs the
//! loop as a stage (task #292). Everything else is the plain command path the
//! stage always had.

use super::Token;
use crate::error::parse::ParseError;
use crate::parse::command::parse_command;
use crate::parse::dispatch::dispatch_keyword;
use crate::parse::heredoc::{HeredocBody, operator_count};
use crate::parse::semi::verbatim;
use crate::parse::word_quoted::span_word;
use crate::parse::{BlockStage, ParsedLine, Stage};
use error_stack::Report;
use sys::{Position, ScriptText};

/// Parse the stage `raw[start..end]`, **untrimmed**: a block stage's own `;`
/// are clause bytes inside it, and its share of the heredoc specs is counted on
/// the whole slice (`trim_at_semi` would stop at the block's first `;` and
/// desync every later stage's spec share). A simple command is trimmed at its
/// first `;` as before: beyond it lie heredoc body words, not command tokens.
pub(super) fn parse_stage(
    raw: &[Token],
    line: &[u8],
    specs: &[HeredocBody],
    spec_at: &mut usize,
    range: core::ops::Range<usize>,
    set_at: Position,
    text: &ScriptText,
) -> Result<Stage, Report<ParseError>> {
    let tokens = raw.get(range).ok_or(ParseError::ExpectedCommandAfterPipe)?;
    let parsed = dispatch_keyword(tokens, text)?;
    match parsed {
        Some(parsed) => block_stage(tokens, parsed, line, spec_at, text),
        None => command_stage(tokens, line, specs, spec_at, set_at, text),
    }
}

/// The keyword-block stage: the parsed block plus its own text (the stage
/// sliced verbatim to its token range), which is what the forked child re-enters
/// through `run::run_parsed`. The block's parsers truncate at their closing
/// keyword, so a heredoc body or a redirect written behind the closer drops out
/// of the block exactly as it does for a statement-level block.
fn block_stage(
    tokens: &[Token],
    parsed: ParsedLine,
    line: &[u8],
    spec_at: &mut usize,
    text: &ScriptText,
) -> Result<Stage, Report<ParseError>> {
    *spec_at += operator_count(line, tokens);
    Ok(Stage::Block(BlockStage {
        parsed,
        text: verbatim(text, tokens)?,
    }))
}

/// The simple-command stage: today's path, with the stage's own heredoc specs,
/// plus its own text slice — the same `verbatim` read the block arm does, on the
/// tokens this stage owns (task #481: the run layer's dispatch decision needs the
/// stage's text to resolve a function name).
fn command_stage(
    tokens: &[Token],
    line: &[u8],
    specs: &[HeredocBody],
    spec_at: &mut usize,
    set_at: Position,
    text: &ScriptText,
) -> Result<Stage, Report<ParseError>> {
    let trimmed = trim_at_semi(tokens);
    let count = operator_count(line, trimmed);
    let stage_specs = specs
        .get(*spec_at..*spec_at + count)
        .ok_or(ParseError::Never)?;
    *spec_at += count;
    let command = parse_command(trimmed, line, stage_specs, set_at)?;
    Ok(Stage::Command(command, verbatim(text, trimmed)?))
}

/// Drop the tokens after the first `;`: heredoc body words are not
/// command tokens. The `;` is [`span_word`]'s, so a quoted `";"` is a word of
/// the stage and cuts nothing (task #336) — the stage keeps the here-doc
/// operator written behind it.
fn trim_at_semi(tokens: &[Token]) -> &[Token] {
    let end = tokens
        .iter()
        .position(|tok| span_word(tok, b";"))
        .unwrap_or(tokens.len());
    tokens.get(..end).unwrap_or(tokens)
}

#[cfg(test)]
mod tests;
