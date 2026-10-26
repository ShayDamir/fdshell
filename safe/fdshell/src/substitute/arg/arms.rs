//! The arms of the single-argument substitution loop: one `impl Ex` block, one
//! method per arm. Each arm owns the bookkeeping its substitution needs — the
//! `before` marker, the mask `realign`, the `idx` re-sync to the stream, the
//! `origin` record — so the loop body in `arg.rs` is one call per byte. Every
//! arm works on [`Ex::parts`], the loop's own fields under the loop's own
//! names, so the arithmetic of the moved code is the loop's, byte for byte: no
//! arm is reordered, no cursor advance changes.

use error_stack::{Report, ResultExt};

use crate::error::resolve::ResolveError;
use crate::mask::quoted_at;
use crate::mask::region_len;
use crate::substitute::backtick;
use crate::substitute::dollar::{self, Region};
use crate::substitute::mask::{push_byte, realign};
use crate::substitute::percent::percent_subst;
use crate::substitute::subst_paren;

use super::Ex;
use super::escape;

impl Ex<'_> {
    /// One byte of the word, dispatched to its arm. `quoted` is the trigger
    /// byte's mask bit and picks the `\` arm; `limit` caps a name arm's read.
    pub(super) fn dispatch(&mut self, b: u8) -> Result<(), Report<ResolveError>> {
        let quoted = quoted_at(self.mask, self.idx);
        // A name arm (`$name`, `%name`, `$N`) reads at most the quote region of
        // its own trigger byte: `idx` is that byte's word index here, before
        // the arm's peek-ahead moves it.
        let limit = region_len(self.mask, self.idx).saturating_sub(1);
        self.idx += 1;
        match b {
            b'\\' => self.backslash(quoted)?,
            b'%' => self.percent(limit, quoted)?,
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

    /// The capture arms' bookkeeping: record `[before, out.len())` as expansion
    /// bytes and re-sync `idx` to the stream. The mask is not padded — the
    /// handler appended a bit for every byte of its own output.
    fn record(&mut self, before: usize) {
        self.origin.record(before, self.out.len());
        self.idx = self.peek.consumed();
    }

    /// The `%` and `$` arms' bookkeeping: pad `out_mask` to the output's length
    /// and re-sync `idx` to the stream, then record `[before, out.len())` as
    /// expansion bytes. `before` is the output length at the arm's start,
    /// `quoted` the trigger byte's mask bit, inherited by every output byte.
    fn resync(&mut self, before: usize, quoted: bool) {
        let (peek, idx, out, out_mask, origin, ..) = self.parts();
        realign(idx, peek, out_mask, before, out, quoted);
        origin.record(before, out.len());
    }

    /// The two `\` arms: POSIX 2.2.1 outside quotes, 2.2.2 inside — `quoted`
    /// picks which. Both keep `idx` synced to the bytes they consume from
    /// `peek`, so the loop's mask indexing stays aligned.
    fn backslash(&mut self, quoted: bool) -> Result<(), Report<ResolveError>> {
        let (peek, idx, out, out_mask, ..) = self.parts();
        if quoted {
            escape::quoted(peek, idx, out, out_mask, quoted)
        } else {
            escape::unquoted(peek, idx, out, out_mask)
        }
    }

    /// The `%` arm: `%name` fd substitution. The arm consumes the trigger byte
    /// and at most the name; a following `%` or non-name byte is literal, so
    /// `%%` reaches argv intact — the `printf` builtin owns `%%` (task #155).
    /// The state borrow ends before the mask realign, and `realign` keeps the
    /// mask 1:1 for the literal `%`.
    fn percent(&mut self, limit: usize, quoted: bool) -> Result<(), Report<ResolveError>> {
        let (peek, _, out, _, _, cell, cache, _) = self.parts();
        let before = out.len();
        let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
        percent_subst(peek, cache, &state, out, limit)?;
        self.resync(before, quoted);
        Ok(())
    }

    /// The `$( … )` arm, its `(` eaten by the loop's guard: the capture
    /// machinery appends its own mask bits, so only `record` is left. The
    /// trigger byte's mask bit protects the output from splitting.
    fn paren(&mut self, quoted: bool) -> Result<(), Report<ResolveError>> {
        let (peek, _, out, out_mask, _, cell, _, _) = self.parts();
        let before = out.len();
        subst_paren::handle_dollar_paren(peek, cell, out, out_mask, quoted)?;
        self.record(before);
        Ok(())
    }

    /// The `` ` … ` `` arm: the same capture machinery as `$( … )`, and the
    /// same record. The trigger byte's mask bit protects the output from
    /// splitting.
    fn tick(&mut self, quoted: bool) -> Result<(), Report<ResolveError>> {
        let (peek, _, out, out_mask, _, cell, _, _) = self.parts();
        let before = out.len();
        backtick::handle(peek, cell, out, out_mask, quoted)?;
        self.record(before);
        Ok(())
    }

    /// The `$` arm (the `$( … )` arm is above): `quoted` is the trigger byte's
    /// mask bit, the quote bit the `$@` arm gives its positionals; `limit` caps
    /// the name read. `region` bundles the mask, the word index of the byte
    /// after `$` (the `{` of a `${…}` arm) and that cap. The `$@` arm's
    /// positional boundaries are recorded by `dollar_subst` itself, through
    /// `origin`; the output range is recorded by `resync`, after the realign.
    fn dollar(&mut self, limit: usize, quoted: bool) -> Result<(), Report<ResolveError>> {
        let (mask, start, limit) = (self.mask, self.idx, limit);
        let region = Region { mask, start, limit };
        let (peek, _, out, _, origin, cell, _, join) = self.parts();
        let before = out.len();
        dollar::dollar_subst(peek, cell, out, region, quoted, join, origin)?;
        self.resync(before, quoted);
        Ok(())
    }
}
