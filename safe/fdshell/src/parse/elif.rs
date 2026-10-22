use super::Token;
use super::cond_bodies::condition_bodies;
use super::if_block::ElifArm;
use super::part_text::part_text;
use super::semi::amp_semi;
use super::semi::block_body;
use super::semi::block_condition;
use super::semi::body_end;
use super::semi::condition_background;
use super::semi::verbatim;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ScriptText;

pub fn parse_elifs(
    tokens: &[Token],
    elif_pairs: &[(usize, usize)],
    else_idx: Option<usize>,
    fi_idx: usize,
    text: &ScriptText,
) -> Result<Vec<ElifArm>, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    elif_pairs
        .iter()
        .enumerate()
        .map(|(i, &(ei, ti))| {
            // The token before a clause keyword is POSIX's `;` and drops out;
            // a plain `&` stays (task #354: `elif true & then …`, and a body
            // whose last statement the `&` backgrounds), and so does a `;`
            // glued behind that `&` (task #355). The arm's own condition flag
            // is read off this same slice (task #356).
            let cond = tokens
                .get(ei + 1..body_end(tokens, line, ti))
                .ok_or(ParseError::MissingCondition)?;
            if ends_in_plain_semi(cond, line) {
                bail!(ParseError::MalformedIfBlock);
            }
            // An empty condition (`elif ; then`, `elif & then`, task #299) is a
            // parse error, and the plain-`;` check above keeps its precedence:
            // the `elif ;; then` slice stays `malformed if block`.
            let cond_trimmed = block_condition(cond, line, ParseError::MissingCondition)?;
            let ec = verbatim(text, cond_trimmed)?;
            let cond_bodies = condition_bodies(text, cond_trimmed);
            let next = elif_pairs
                .get(i + 1)
                .map(|&(ne, _)| ne)
                .or(else_idx)
                .unwrap_or(fi_idx);
            let raw = tokens
                .get(ti + 1..body_end(tokens, line, next))
                .unwrap_or(&[]);
            if ends_in_plain_semi(raw, line) {
                bail!(ParseError::MalformedIfBlock);
            }
            let eb = part_text(text, block_body(raw, line, ParseError::EmptyElifBody)?)?;
            Ok(ElifArm {
                cond: ec,
                body: eb,
                cond_bodies,
                cond_background: condition_background(cond, line),
            })
        })
        .collect::<Result<Vec<_>, Report<ParseError>>>()
}

pub fn parse_else_body(
    tokens: &[Token],
    else_idx: usize,
    fi_idx: usize,
    text: &ScriptText,
) -> Result<ScriptText, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let raw = tokens
        .get(else_idx + 1..body_end(tokens, line, fi_idx))
        .unwrap_or(&[]);
    if ends_in_plain_semi(raw, line) {
        bail!(ParseError::MalformedIfBlock);
    }
    part_text(text, block_body(raw, line, ParseError::EmptyElseBody)?)
}

/// The slice's last token is a `;` **not** glued behind a plain `&`: a body
/// that ends in POSIX's bare `;` before a clause keyword is the malformed
/// arm; the `& ;` shape is not (its body text carries the split the body's
/// own scan reports, task #355 — the arm must reach that scan, reporting
/// `unexpected character '&'` like every other block, STYLE §4.7).
fn ends_in_plain_semi(slice: &[Token], line: &[u8]) -> bool {
    let Some(last) = slice.len().checked_sub(1) else {
        return false;
    };
    slice
        .last()
        .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
        && !amp_semi(slice, line, last)
}
