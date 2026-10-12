//! The segment scanner's state: its cursor, its per-line bookkeeping, and the
//! segments and runs it is building. Grouped so each open branch (a keyword
//! block, a `{ …; }` group) takes the state as one argument and rewinds it
//! itself, keeping `segment.rs` a plain scan loop.

use crate::scan::ScanState;
use crate::segment::Segment;
use alloc::vec::Vec;

/// The scanner's cursor (`i`, `start`), the physical line's start, the resume
/// floor (after a block, no flush may resume inside a heredoc body region of
/// the block's line, or of a statement that precedes the block on that line),
/// the tokenizer state, the segments collected and the runs buffered for the
/// next flush.
pub(super) struct Book<'a> {
    pub(super) i: usize,
    pub(super) start: usize,
    pub(super) line_start: usize,
    pub(super) body_floor: usize,
    pub(super) state: ScanState,
    pub(super) segments: Vec<Segment<'a>>,
    pub(super) runs: Vec<(&'a [u8], usize)>,
}

impl<'a> Book<'a> {
    /// A book positioned at the start of the line, with nothing collected.
    pub(super) fn new() -> Self {
        Self {
            i: 0,
            start: 0,
            line_start: 0,
            body_floor: 0,
            state: ScanState::new(),
            segments: Vec::new(),
            runs: Vec::new(),
        }
    }

    /// Rewind the cursor to `resume` — just past a block's span (a trailing
    /// statement on the same line is still scanned) — with the line start the
    /// trailing runs' bookkeeping continues from and the resume floor for the
    /// physical line (`None` keeps the current floor).
    pub(super) fn rewind(&mut self, resume: usize, line_start: usize, floor: Option<usize>) {
        self.i = resume;
        self.start = resume;
        self.line_start = line_start;
        self.body_floor = floor.unwrap_or(self.body_floor);
    }
}
