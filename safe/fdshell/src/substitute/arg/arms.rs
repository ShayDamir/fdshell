//! The dispatch of the single-argument substitution loop, with the mask
//! helpers its byte arm uses; the arm methods and the bookkeeping they share
//! are in [`handlers`] (task #576 split). Every arm works on [`Ex::parts`], the
//! loop's own fields under the loop's own names, so the arithmetic of the moved
//! code is the loop's, byte for byte: no arm is reordered, no cursor advance
//! changes.

// mutants: -C --lib -C --test -C colon -C --test -C unset_expand -C --test -C positional -C --test -C arith -C --test -C quotes -- -E 'test(/^substitute::/) | binary(colon) | binary(unset_expand) | binary(positional) | binary(arith) | binary(quotes)'

mod handlers;

use error_stack::Report;

use crate::error::resolve::ResolveError;
use crate::mask::quoted_at;
use crate::mask::region_len;
use crate::substitute::mask::push_byte;
use crate::substitute::surface::PercentArm;

use super::Ex;

impl Ex<'_> {
    /// One byte of the word, dispatched to its arm. `quoted` is the trigger
    /// byte's mask bit: it picks the `\` arm and gates the `%` arm on the bit
    /// (task #252 — a quoted `%` is an ordinary literal byte, the same shape
    /// LESSONS "a quoted region is a unit for *every* byte scanner" prescribes);
    /// `self.percent` is the per-surface policy of task #371. `limit` caps a
    /// name arm's read.
    pub(super) fn dispatch(&mut self, b: u8) -> Result<(), Report<ResolveError>> {
        let quoted = quoted_at(self.mask, self.idx);
        // A name arm (`$name`, `%name`, `$N`) reads at most the quote region of
        // its own trigger byte: `idx` is that byte's word index here, before
        // the arm's peek-ahead moves it.
        let limit = region_len(self.mask, self.idx).saturating_sub(1);
        self.idx += 1;
        match b {
            b'\\' => self.backslash(quoted)?,
            b'%' if !quoted && self.percent == PercentArm::Fires => self.percent(limit)?,
            b'$' if self.peek.peek() == Some(&b'(') => {
                self.peek.next();
                self.paren(quoted)?;
            }
            b'`' => self.tick(quoted)?,
            b'$' => self.dollar(limit, quoted)?,
            _ => push_byte(&mut self.out, &mut self.out_mask, b, quoted)?,
        }
        Ok(())
    }
}
