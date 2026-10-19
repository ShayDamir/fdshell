//! The segment scanner's state: its cursor, its per-line bookkeeping, and the
//! segments and runs it is building. Grouped so each open branch (a keyword
//! block, a `{ …; }` group) takes the state as one argument and rewinds it
//! itself, keeping `segment.rs` a plain scan loop.

use crate::scan::Boundary;
use crate::scan::ScanState;
use crate::segment::Segment;
use alloc::vec::Vec;

/// A buffered run: its trimmed bytes, the offset of its first byte, and
/// whether the plain `&` marked it to run asynchronously (task #326).
pub(super) struct Run<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) off: usize,
    pub(super) background: bool,
}

/// The scanner's cursor (`i`, `start`), the physical line's start, the resume
/// floor (after a block, no flush may resume inside a heredoc body region of
/// the block's line, or of a statement that precedes the block on that line),
/// the tokenizer state, the segments collected and the runs buffered for the
/// next flush. `sep_since_run` records that a `;`-separated run was pushed
/// since the last run push: an async `&` may not target a run the `;` already
/// terminated (`echo a ; &` — a split-time validation input, task #326).
pub(super) struct Book<'a> {
    pub(super) i: usize,
    pub(super) start: usize,
    pub(super) line_start: usize,
    pub(super) body_floor: usize,
    pub(super) state: ScanState,
    pub(super) segments: Vec<Segment<'a>>,
    pub(super) runs: Vec<Run<'a>>,
    pub(super) sep_since_run: bool,
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
            sep_since_run: false,
        }
    }

    /// Resume the cursor past the separator at `sep`: a `#` comment ends the
    /// physical line (the runs' line bookkeeping restarts at its end and the
    /// `;` chapter closes, so the runs stay a plain-`&` target), any other
    /// separator starts the next run just past it. A flush that resumed
    /// further on (a heredoc body region, a block's line floor) wins, and
    /// never adds a second byte on top of it.
    pub(super) fn resume_past(&mut self, sep: usize, kind: Boundary, deferred: bool, line: &[u8]) {
        if kind == Boundary::Comment {
            self.i = crate::scan::skip_comment(line, self.i);
            self.start = self.i;
            self.line_start = self.i;
            self.sep_since_run = false;
            return;
        }
        if kind == Boundary::Separator {
            self.sep_since_run = !deferred;
        }
        self.i = self.i.max(sep + 1);
        self.start = self.i;
    }

    /// Rewind the cursor to `resume` — just past a block's span (a trailing
    /// statement on the same line is still scanned) — with the line start the
    /// trailing runs' bookkeeping continues from and the resume floor for the
    /// physical line.
    pub(super) fn rewind(&mut self, resume: usize, line_start: usize, floor: usize) {
        self.i = resume;
        self.start = resume;
        self.line_start = line_start;
        self.body_floor = floor;
    }
}
