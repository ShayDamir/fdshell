//! Whole-text scan for the opaque heredoc body regions, so the tokenizer can
//! blank body bytes before tokenizing (a body may contain any bytes,
//! including unbalanced quotes).

use super::super::{Boundary, ScanState, boundary, skip_comment};
use super::{body_spans, line_end_after, operator_count, operator_delims, skip_region};
use alloc::vec::Vec;

/// The opaque body regions of the text, in order: for every heredoc command
/// line — top-level or inside a block body — the span from the first body
/// byte to the start of the last delimiter line. The tokenizer blanks these
/// spans so body bytes never tokenize.
///
/// A command line ends at a newline **or** at a `;`: the latter is a
/// single-line keyword block's arm (`if true; then cat <<T; fi`), whose body
/// sits past the closing keyword, on the lines behind the `;`. Such a run is
/// resolved from its physical line's end ([`semi_terminated_regions`]); a
/// newline-terminated run is resolved by [`skip_region`] from its own end.
pub(crate) fn body_regions(line: &[u8]) -> Vec<(usize, usize)> {
    let mut regions: Vec<(usize, usize)> = Vec::new();
    let mut state = ScanState::new();
    let mut run_start = 0usize;
    let mut i = 0usize;
    while i <= line.len() {
        let kind = boundary(line, i, &state);
        if kind == Boundary::Char {
            i = state.advance(line, i);
            continue;
        }
        state.word_active = false;
        if kind == Boundary::Comment {
            i = skip_comment(line, i);
            run_start = i;
            continue;
        }
        match skip_region(line, run_start, i) {
            Some((region, resume, _)) => {
                regions.push(region);
                i = resume;
                run_start = i;
            }
            None => {
                if let Some((regions_of_line, resume)) = semi_terminated_regions(line, run_start, i)
                {
                    regions.extend(regions_of_line);
                    i = resume;
                    run_start = i;
                    continue;
                }
                i += 1;
                run_start = i;
            }
        }
    }
    regions
}

/// The regions of a heredoc command line terminated by `;` — a single-line
/// block's arm (`if true; then cat <<T; fi`), whose body sits past the closing
/// keyword, on the lines *behind* the `;`. [`skip_region`] only knows
/// newline-terminated command lines, so the body is resolved from the
/// **physical line's** end: every `<<` operator the line writes claims one
/// region, in operator order, as the segment layer's `line_bodies_for_line`
/// does (one rule at both granularities, LESSONS). The run behind the `;` is
/// consumed by the returned resume index, so the line's regions are counted
/// once. `None` when the separator is not a `;`, the run writes no operator,
/// or a delimiter line is missing (the parse layer's own error, as before).
fn semi_terminated_regions(
    line: &[u8],
    run_start: usize,
    semi: usize,
) -> Option<(Vec<(usize, usize)>, usize)> {
    if line.get(semi) != Some(&b';') || operator_count(line, run_start, semi) == 0 {
        return None;
    }
    let start = line_start(line, run_start);
    let body_start = line_end_after(line, semi);
    let ops = operator_delims(line, start, body_start)?;
    if ops.is_empty() {
        return None;
    }
    body_spans(line, body_start, &ops).ok()
}

/// The start of the physical line that contains `pos`.
fn line_start(line: &[u8], pos: usize) -> usize {
    line.get(0..pos)
        .and_then(|head| head.iter().rposition(|&b| b == b'\n').map(|n| n + 1))
        .unwrap_or(0)
}
