//! The `(( … ))` region of the arithmetic `for`: its raw-byte span and the
//! three clauses cut at its top-level `;` (rules 2 and 3 of `arith.rs`).

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::scan::{Boundary, ScanState, boundary};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, ensure};
use sys::ScriptText;

/// The measured region: the interior bytes and the byte just past the `))`.
pub(crate) struct Region {
    /// Byte index of the interior's first byte, just past the opening `((`.
    inner: usize,
    /// Interior length in bytes, the closing `))` excluded.
    len: usize,
    /// Byte index just past the closing `))`: the `do … done` tail starts here.
    /// Read by `arith.rs`, the only caller of [`measure`].
    pub(super) end: usize,
}

/// The byte index of the `((` opening the arithmetic `for`, or `None` when the
/// `for` keyword word is not followed by `((` (the gap is whitespace-only:
/// `for ((`, `for\t((`, `for  ((`; no gap inside the pair).
pub(crate) fn open_at(raw: &[Token], line: &[u8]) -> Option<usize> {
    let after = raw.first()?.2;
    let open = after
        + line
            .get(after..)?
            .iter()
            .take_while(|b| b.is_ascii_whitespace())
            .count();
    (line.get(open..open + 2)? == *b"((").then_some(open)
}

/// Measure the region starting at `open`. `Err` when the line ends inside it
/// (bash's `for ((i=0;i<2`), `Ok(None)` when the depth return is not the `))`
/// the form needs (`for (();do;done`: a `(())` pair is not an `(( … ))` region).
pub(crate) fn measure(line: &[u8], open: usize) -> Result<Option<Region>, Report<ParseError>> {
    // `advance` consumes the `((` pair as one unit and opens the depth; the walk
    // stops where the depth returns, i.e. just past the closing pair's first
    // `)`. Every byte it passes — `;`, `|`, `&&`, `<<`, a newline, a quote — is
    // data: the region is opaque, the rule `scan/advance` already applies to the
    // statement splitter and to the cond-list splitter.
    let mut state = ScanState::new();
    let mut i = state.advance(line, open);
    while i < line.len() && state.paren_depth > 0 {
        i = state.advance(line, i);
    }
    ensure!(
        state.paren_depth == 0,
        ParseError::ArithForMissingCloseParen
    );
    // `i` is the byte after the closing pair's first `)`, so that `)` is `i - 1`
    // and the interior ends there; the pair's second `)` must sit at `i`, and
    // the tail starts on the byte after it.
    if line.get(i) != Some(&b')') {
        return Ok(None);
    }
    Ok(Some(Region {
        inner: open + 2,
        len: i - (open + 3),
        end: i + 1,
    }))
}

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
pub(crate) fn clauses(
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
