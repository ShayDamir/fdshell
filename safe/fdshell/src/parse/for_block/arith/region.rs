//! The `(( … ))` region of the arithmetic `for`: its raw-byte span and the
//! three clauses cut at its top-level `;` (rules 2 and 3 of `arith.rs`).

mod clause;

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::scan::ScanState;
pub(super) use clause::clauses;
use error_stack::{Report, ensure};

/// The measured region: the interior bytes and the byte just past the `))`.
pub(crate) struct Region {
    /// Byte index of the interior's first byte, just past the opening `((`.
    /// Read by the clause cut in the `clause` module.
    pub(super) inner: usize,
    /// Interior length in bytes, the closing `))` excluded. Read by the
    /// `clause` module like [`Self::inner`].
    pub(super) len: usize,
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
