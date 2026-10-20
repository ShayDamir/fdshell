//! Whole-text scan for the opaque heredoc body regions, so the tokenizer can
//! blank body bytes before tokenizing (a body may contain any bytes,
//! including unbalanced quotes). The delimiter lines are the second half of the
//! same scan ([`terminator_spans`]): one definition of what is heredoc
//! structure, so an opacity change cannot drift between the two halves.

use super::super::{Boundary, ScanState, boundary, skip_comment};
use super::{body_spans, line_end_after, line_start, operator_count, operator_delims, skip_region};
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

/// The delimiter (terminator) lines of the text's heredocs: the span of each
/// delimiter line's **bytes**, its terminating newline excluded (an empty
/// delimiter line yields no span). Derived from [`body_regions`], so the body
/// and its terminator line are one scan and cannot drift: a terminator line is
/// heredoc structure, never command bytes — an operator count must not see its
/// bytes, and its quote bytes must never reach the tokenizer's quote check.
/// The line is scanned as opaque data (a terminator may carry an unbalanced
/// quote byte), so the newline search is a plain byte scan.
pub(crate) fn terminator_spans(line: &[u8]) -> Vec<(usize, usize)> {
    body_regions(line)
        .into_iter()
        .map(|(_start, term_start)| (term_start, terminator_end(line, term_start)))
        .filter(|(start, stop)| stop > start)
        .collect()
}

/// The end of the delimiter line that starts at `start`: the index of its
/// terminating newline, or `line.len()` when it is the text's last line.
pub(crate) fn terminator_end(line: &[u8], start: usize) -> usize {
    line.get(start..)
        .and_then(|s| s.iter().position(|&b| b == b'\n'))
        .map_or(line.len(), |p| start + p)
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
pub(super) fn semi_terminated_regions(
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
    // Live guard, and it needs no window-subset argument any more: `operator_count`
    // counts the same sites `operator_delims` reads (task #261), so an empty `ops`
    // with a non-empty run means the *line* window hides the operator that the run
    // window sees — an unclosed `((` (or `$(`) region: `(( x ; cat <<B; fi` counts 1
    // in the `cat <<B` run and 0 over the line. The guard is what keeps that
    // operator from jumping `i` to `body_start` and skipping the line's runs behind
    // the `;`; pinned in `scan/heredoc/tests.rs::semi_terminated_regions_guard_is_live`.
    // LESSONS: a guard no input can reach is recorded, not chased.
    if ops.is_empty() {
        return None;
    }
    body_spans(line, body_start, &ops).ok()
}
