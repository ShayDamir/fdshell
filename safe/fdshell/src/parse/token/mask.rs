//! The quote-mask bits and the quote-region bookkeeping of the tokenizer.
//!
//! These are the only places a word's mask bit is ever produced, so the rule
//! for where a **quote region** starts lives in one spot: the first byte inside
//! an open pair of quote bytes opens the region (`QuotedStart`) and the bytes
//! that follow continue it (`Quoted`). A region closes at [`State::close_span`]
//! (a `"` or `'` closed), at a word boundary, and at every unquoted byte, so
//! adjacent regions (`"$v""b"`, `"$v"'x'y`) are distinguishable in the mask
//! while a single `bool` per byte reads them as one region (task #285).

use super::State;
use crate::error::parse::ParseError;
use crate::mask::MaskBit;
use error_stack::{Report, ResultExt};

impl State {
    /// One word byte with its quote state: the first byte of a quote region is
    /// a region start, an unquoted byte closes any region left open.
    pub(in crate::parse) fn push_masked(
        &mut self,
        b: u8,
        quoted: bool,
    ) -> Result<(), Report<ParseError>> {
        let bit = if !quoted {
            self.region_open = false;
            MaskBit::Unquoted
        } else if self.region_open {
            MaskBit::Quoted
        } else {
            self.region_open = true;
            MaskBit::QuotedStart
        };
        self.cur
            .push_byte(b)
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        self.mask.push(bit);
        Ok(())
    }

    /// A run of bytes with the same quote state — a `$( … )` / `${ … }` /
    /// `` ` … ` `` region body, a `'…'` span: the region-start rule applies to
    /// the first byte, the rest continue the region it opened.
    pub(in crate::parse) fn push_masked_run(
        &mut self,
        bytes: &[u8],
        quoted: bool,
    ) -> Result<(), Report<ParseError>> {
        let Some((first, rest)) = bytes.split_first() else {
            return Ok(());
        };
        self.push_masked(*first, quoted)?;
        for &b in rest {
            self.push_masked(b, quoted)?;
        }
        Ok(())
    }

    /// A quote pair just closed, so the next quoted byte starts a new region.
    /// If the pair contributed no byte (`cur` is exactly as long as when it
    /// opened), its opening index goes into `spans` — the mask has no bit for a
    /// span of zero bytes, and with it no region start (task #321).
    pub(in crate::parse) fn close_span(&mut self) {
        self.region_open = false;
        if self.cur.len() == self.span_open {
            self.spans.push(self.span_open);
        }
    }
}
