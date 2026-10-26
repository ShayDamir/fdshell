//! The region's **clauses**: the top-level `;` cut points and the three-clause
//! split (rule 3 of `arith.rs`). Extracted from
//! `parse/for_block/arith/region.rs` for the line budget (task #456); the byte
//! span measurement (`measure`) stays with the region itself.

use super::Region;
use crate::error::parse::ParseError;
use crate::scan::{Boundary, ScanState, boundary};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, ensure};
use sys::ScriptText;

/// The interior's top-level `;` byte indices. A newline is **not** one: bash
/// reads `for ((i=0\ni<2\ni++))` as an arithmetic syntax error and
/// `for ((i=0;\ni<2;\ni++))` as the loop.
fn cut_points(line: &[u8], region: &Region) -> Vec<usize> {
    let mut state = ScanState::new();
    let mut points = Vec::new();
    let end = region.inner + region.len;
    let mut i = region.inner;
    while i < end {
        // [`boundary`]'s separator is a `;` **or** a newline at this depth; only
        // the `;` byte cuts a clause.
        if line.get(i) == Some(&b';') && boundary(line, i, &state) == Boundary::Separator {
            points.push(i);
        }
        i = state.advance(line, i);
    }
    points
}

/// The interior's three clauses: exactly two top-level `;`, since bash rejects
/// a one- and a four-clause region (`((i=0))`, `((i=0;i<2;i++;))`) the same way.
/// A clause written empty is `None`: an empty condition is true, an empty init
/// and an empty step are no-ops.
pub(in crate::parse::for_block::arith) fn clauses(
    text: &ScriptText,
    region: &Region,
) -> Result<[Option<ScriptText>; 3], Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let mut points = cut_points(line, region);
    ensure!(points.len() == 2, ParseError::ArithForClauseCount);
    let second = points.pop().ok_or(ParseError::Never)?;
    let first = points.pop().ok_or(ParseError::Never)?;
    let last = region.inner + region.len;
    Ok([
        clause(text, line, region.inner, first)?,
        clause(text, line, first + 1, second)?,
        clause(text, line, second + 1, last)?,
    ])
}

/// One clause's text, whitespace-trimmed; `None` when it was written empty.
fn clause(
    text: &ScriptText,
    line: &[u8],
    start: usize,
    end: usize,
) -> Result<Option<ScriptText>, Report<ParseError>> {
    let seg = line.get(start..end).ok_or(ParseError::Never)?;
    let lead = seg.len() - seg.trim_ascii_start().len();
    let trimmed = seg.trim_ascii();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        text.subslice(start + lead, trimmed.len())
            .ok_or(ParseError::Never)?,
    ))
}
