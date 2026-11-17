//! The arm methods of [`super`]'s dispatch — one `impl Ex` block, one method
//! per arm (task #576 split). Each arm owns the bookkeeping its substitution
//! needs — the `before` marker, the mask `realign`, the `idx` re-sync to the
//! stream, the `origin` record — so the loop body in `arg.rs` is one call per
//! byte.

// mutants: -C --lib -C --test -C colon -C --test -C unset_expand -C --test -C positional -C --test -C arith -C --test -C quotes -- -E 'test(/^substitute::/) | binary(colon) | binary(unset_expand) | binary(positional) | binary(arith) | binary(quotes)'

use error_stack::{Report, ResultExt};

use crate::error::resolve::ResolveError;
use crate::substitute::backtick;
use crate::substitute::dollar::{self, Region};
use crate::substitute::mask::realign;
use crate::substitute::percent::percent_subst;
use crate::substitute::subst_paren;

use crate::substitute::arg::Ex;
use crate::substitute::arg::escape;

impl Ex<'_> {
    /// The capture arms' bookkeeping: record `[before, out.len())` as expansion
    /// bytes and re-sync `idx` to the stream. The mask is not padded — the
    /// handler appended a bit for every byte of its own output.
    pub(super) fn record(&mut self, before: usize) {
        self.origin.record(before, self.out.len());
        self.idx = self.peek.consumed();
    }

    /// The `%` and `$` arms' bookkeeping: pad `out_mask` to the output's length
    /// and re-sync `idx` to the stream, then record `[before, out.len())` as
    /// expansion bytes. `before` is the output length at the arm's start,
    /// `quoted` the trigger byte's mask bit, inherited by every output byte.
    pub(super) fn resync(&mut self, before: usize, quoted: bool) {
        let (peek, idx, out, out_mask, origin, ..) = self.parts();
        realign(idx, peek, out_mask, before, out, quoted);
        origin.record(before, out.len());
    }

    /// The two `\` arms: POSIX 2.2.1 outside quotes, 2.2.2 inside — `quoted`
    /// picks which. Both keep `idx` synced to the bytes they consume from
    /// `peek`, so the loop's mask indexing stays aligned.
    pub(super) fn backslash(&mut self, quoted: bool) -> Result<(), Report<ResolveError>> {
        let (peek, idx, out, out_mask, ..) = self.parts();
        if quoted {
            escape::quoted(peek, idx, out, out_mask, quoted)
        } else {
            escape::unquoted(peek, idx, out, out_mask)
        }
    }

    /// The `%` arm: `%name` fd substitution. The dispatch guard runs it only
    /// for an **unquoted** trigger byte on a surface where the arm is live
    /// (tasks #252, #371), so `quoted` is always false here and every output
    /// byte carries the unquoted bit. The arm consumes the trigger byte
    /// and at most the name; a following `%` or non-name byte is literal, so
    /// `%%` reaches argv intact — the `printf` builtin owns `%%` (task #155).
    /// The state borrow ends before the mask realign, and `realign` keeps the
    /// mask 1:1 for the literal `%`.
    pub(super) fn percent(&mut self, limit: usize) -> Result<(), Report<ResolveError>> {
        let (peek, _, out, _, _, cell, cache, _) = self.parts();
        let before = out.len();
        let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
        percent_subst(peek, cache, &state, out, limit)?;
        self.resync(before, false);
        Ok(())
    }

    /// The `$( … )` arm, its `(` eaten by the loop's guard: the capture
    /// machinery appends its own mask bits, so only `record` is left. The
    /// trigger byte's mask bit protects the output from splitting.
    pub(super) fn paren(&mut self, quoted: bool) -> Result<(), Report<ResolveError>> {
        let (peek, _, out, out_mask, _, cell, _, _) = self.parts();
        let before = out.len();
        subst_paren::handle_dollar_paren(peek, cell, out, out_mask, quoted)?;
        self.record(before);
        Ok(())
    }

    /// The `` ` … ` `` arm: the same capture machinery as `$( … )`, and the
    /// same record. The trigger byte's mask bit protects the output from
    /// splitting.
    pub(super) fn tick(&mut self, quoted: bool) -> Result<(), Report<ResolveError>> {
        let (peek, _, out, out_mask, _, cell, _, _) = self.parts();
        let before = out.len();
        backtick::handle(peek, cell, out, out_mask, quoted)?;
        self.record(before);
        Ok(())
    }

    /// The `$` arm (the `$( … )` arm is above): `quoted` is the trigger byte's
    /// mask bit, the quote bit the `$@` arm gives its positionals; `limit` caps
    /// the name read. `region` bundles the mask, the word index of the byte
    /// after `$` (the `{` of a `${…}` arm) and that cap, plus the word's
    /// zero-byte quote spans: an operator word slices all three to its own bytes
    /// before it expands itself (task #561). The `$@` arm's
    /// positional boundaries are recorded by `dollar_subst` itself, through
    /// `origin`; the output range is recorded by `resync`, after the realign.
    pub(super) fn dollar(
        &mut self,
        limit: usize,
        quoted: bool,
    ) -> Result<(), Report<ResolveError>> {
        let (mask, start, limit, spans) = (self.mask, self.idx, limit, self.spans);
        let region = Region {
            mask,
            start,
            limit,
            spans,
        };
        let (peek, _, out, _, origin, cell, _, join) = self.parts();
        let before = out.len();
        dollar::dollar_subst(peek, cell, out, region, quoted, join, origin)?;
        self.resync(before, quoted);
        Ok(())
    }
}
