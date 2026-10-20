mod types;

pub(crate) use types::{ElifArm, IfBlock};

use super::Token;
use super::cond_bodies::condition_bodies;
use super::part_text::part_text;
use super::semi::List;
use super::semi::Term;
use super::semi::block_body;
use super::semi::body_end;
use super::semi::condition_background;
use super::semi::find_preceded_by;
use super::semi::trim_condition;
use super::semi::up_to_closing;
use super::semi::verbatim;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, ensure};
use sys::ScriptText;

pub(crate) fn tokens_to_if(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<IfBlock, Report<ParseError>> {
    ensure!(
        tokens
            .first()
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"if")),
        ParseError::MalformedIfBlock
    );

    // A condition-position heredoc's bodies sit in the block text after the
    // closing `fi`: the tokens after it are body data, not block structure.
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let tokens = up_to_closing(tokens, line, List::Plain, ParseError::MissingFi)?;
    // The closing `fi` is the truncated slice's last token.
    let fi_idx = tokens.len() - 1;

    // An `if`/`elif` condition is a compound list: `then` may sit behind the
    // plain `&` as behind a `;` (task #354), and that `&` is the condition's
    // background-job flag (task #356, `condition_background`).
    let first_then =
        find_preceded_by(tokens, 1, b"then", line, Term::SemiAmp).ok_or(ParseError::MissingThen)?;

    let cond_slice = tokens
        .get(1..first_then)
        .ok_or(ParseError::MissingCondition)?;
    let cond_tokens = trim_condition(cond_slice, line);
    let cond_background = condition_background(cond_slice, line);

    let condition = span_verbatim(
        text,
        tokens,
        1,
        first_then,
        line,
        ParseError::MissingCondition,
    )?;
    let cond_bodies = condition_bodies(text, cond_tokens);

    let mut elif_pairs: Vec<(usize, usize)> = Vec::new();
    let mut pos = first_then;
    while let Some(elif_idx) = find_preceded_by(tokens, pos, b"elif", line, Term::SemiAmp) {
        let then_idx = find_preceded_by(tokens, elif_idx, b"then", line, Term::SemiAmp)
            .ok_or(ParseError::MissingThenAfterElif)?;
        elif_pairs.push((elif_idx, then_idx));
        pos = then_idx;
    }
    let else_idx = find_preceded_by(tokens, pos, b"else", line, Term::SemiAmp);

    let first_end = match elif_pairs.first() {
        Some(&(ei, _)) => ei,
        None => else_idx.unwrap_or(fi_idx),
    };
    // The token before the closer is POSIX's `;` and drops out of the body, a
    // plain `&` stays (it backgrounds the body's last statement, task #354),
    // so does a `;` glued right behind that `&` (the body's own scan rejects
    // the split, task #355), and so does any other word the closer is glued to.
    let raw = tokens
        .get(first_then + 1..body_end(tokens, line, first_end))
        .unwrap_or(&[]);
    let then_body = part_text(text, block_body(raw, line, ParseError::EmptyThenBody)?)?;

    let elifs = super::elif::parse_elifs(tokens, &elif_pairs, else_idx, fi_idx, text)?;
    let else_body = else_idx
        .map(|ei| super::elif::parse_else_body(tokens, ei, fi_idx, text))
        .transpose()?;
    Ok(IfBlock {
        condition,
        then_body,
        elifs,
        else_body,
        cond_bodies,
        cond_background,
    })
}

/// The verbatim text of `tokens[from..to]` as a trimmed condition slice;
/// `err` when the range is absent.
fn span_verbatim(
    text: &ScriptText,
    tokens: &[Token],
    from: usize,
    to: usize,
    line: &[u8],
    err: ParseError,
) -> Result<ScriptText, Report<ParseError>> {
    verbatim(text, trim_condition(tokens.get(from..to).ok_or(err)?, line))
}
