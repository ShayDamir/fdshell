//! Whole-text scan for the opaque heredoc body regions, so the tokenizer can
//! blank body bytes before tokenizing (a body may contain any bytes,
//! including unbalanced quotes).

use super::super::{Boundary, ScanState, boundary, skip_comment};
use super::skip_region;
use alloc::vec::Vec;

/// The opaque body regions of the text, in order: for every heredoc command
/// line — top-level or inside a block body — the span from the first body
/// byte to the start of the last delimiter line. The tokenizer blanks these
/// spans so body bytes never tokenize.
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
                i += 1;
                run_start = i;
            }
        }
    }
    regions
}
