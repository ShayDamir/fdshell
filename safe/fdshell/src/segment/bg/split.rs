//! The split-time decision for one `&`: is there a command to terminate
//! (target), and which run does the `&` mark? Field bundle of the scanner's
//! state at the cursor (the newtype the validation logic reads as methods).

use crate::segment::Segment;
use crate::segment::book::Run;
use alloc::vec::Vec;

/// The scanner state a split decision reads: the line, the cursor at the `&`,
/// the word being scanned (`pending`, trimmed), the buffered runs, the
/// flushed segments, and whether a `;` closed the last run.
pub(super) struct Split<'a, 'b> {
    pub(super) line: &'a [u8],
    pub(super) start: usize,
    pub(super) i: usize,
    pub(super) pending: &'a [u8],
    pub(super) runs: &'b mut Vec<Run<'a>>,
    pub(super) segments: &'b mut Vec<Segment<'a>>,
    pub(super) sep_since_run: bool,
}

impl<'a, 'b> Split<'a, 'b> {
    /// The split is valid when what peeks behind the `&` is not a `;`/`|`/`&`
    /// (bash's "syntax error, nothing runs" family) and the `&` has a command
    /// to terminate (see [`Split::has_target`]).
    pub(super) fn valid(&self) -> bool {
        self.peek().is_none_or(|b| !matches!(b, b';' | b'|' | b'&')) && self.has_target()
    }

    /// Mark the target (see [`Split::has_target`]): the last buffered run; a
    /// word being scanned becomes a marked run (`echo hi &` — its run is not
    /// buffered yet); else the last flushed statement (a multi-line split);
    /// a closed block is marked by nobody (it runs foreground).
    pub(super) fn mark(&mut self) {
        if let Some(run) = self.runs.last_mut() {
            run.background = true;
            return;
        }
        if !self.pending.is_empty() {
            self.runs.push(Run {
                bytes: self.pending,
                off: self.pending_off(),
                background: true,
            });
            return;
        }
        if let Some(Segment::Statement { background, .. }) = self.segments.last_mut() {
            *background = true;
        }
    }

    /// The pending word's offset: `start` plus its leading whitespace.
    fn pending_off(&self) -> usize {
        let raw = self.line.get(self.start..self.i).unwrap_or(b"");
        self.start + raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count()
    }

    /// A `;` past the last buffered run leaves no target for the `&`. The
    /// target is a buffered run, the word being scanned (`echo hi &` splits
    /// before its run is pushed), a statement flushed by the deferral above
    /// (no `;` closed it), or a closed block (`fi &` — foreground: blocks
    /// carry no background flag).
    fn has_target(&self) -> bool {
        if let Some(last) = self.runs.last() {
            return !self.sep_since_run && !last.bytes.is_empty();
        }
        if !self.pending.is_empty() {
            return true;
        }
        match self.segments.last() {
            Some(Segment::Statement { .. }) => !self.sep_since_run,
            Some(Segment::Block { closed: true, .. }) => true,
            _ => false,
        }
    }

    /// The first byte after the `&`, skipping spaces and tabs only — a
    /// newline ends the peek (it passes the check: the multi-line `&\n;`
    /// shape streams like bash, line 1 executes and the `;` line is fdshell's
    /// accepted divergence), and so does end of input.
    fn peek(&self) -> Option<u8> {
        let mut j = self.i + 1;
        while matches!(self.line.get(j), Some(&b' ') | Some(&b'\t')) {
            j += 1;
        }
        self.line.get(j).copied().filter(|b| *b != b'\n')
    }
}

#[cfg(test)]
mod tests;
