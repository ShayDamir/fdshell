//! The word of a braced colon operator: it goes through the **ordinary word
//! pipeline** before the arm uses it (POSIX #2.6.3 — the word of
//! `${name:-word}` undergoes every expansion), while the whole `${ … }` result
//! keeps the outer reference's quote bit. The word's own mask governs only its
//! own expansion: an unquoted region is field-split and globbed **after** the
//! word expanded (bash: `${u:="*"}` globs, `${u:="a b"}` splits, while
//! `"${u:="*"}"` stays one word), and a quoted `"$@"` inside the word keeps its
//! word boundaries. The sub-mask is therefore used inside the word and dropped;
//! only the boundaries travel out.
//! LESSONS: "An operator word is a word"

// mutants: -C --lib -C --test -C colon -C --test -C unset_expand -C --test -C positional -C --test -C arith -C --test -C quotes -- -E 'test(/^substitute::/) | binary(colon) | binary(unset_expand) | binary(positional) | binary(arith) | binary(quotes)'

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::state::ShellState;
use crate::substitute::Join;
use crate::substitute::arg::substitute_arg_marked;
use crate::substitute::origin::Origin;

mod truncated;

/// The word of a colon operator, with the context its expansion needs: the
/// outer word's mask and zero-byte quote spans sliced to the word's own bytes
/// (`start` is the word's index in the outer text — the word text is a subslice
/// of it), the shell state, the `$@`/`$*` join the arm measured, and
/// `force_unquoted`, true only for `:?`, whose message bash expands unquoted
/// even when the reference is quoted (`"${u:?a\ b}"` reports `a b`).
pub(super) struct Word<'a> {
    pub literal: &'a ShortCStr,
    pub mask: &'a [MaskBit],
    pub start: usize,
    pub spans: &'a [usize],
    pub cell: &'a ForkCell<ShellState>,
    pub join: Join,
    pub force_unquoted: bool,
}

/// The word's expansion: its text, plus the `$@` boundaries it produced (its own
/// output mask is deliberately not carried — the caller pads the whole `${ … }`
/// range with the outer trigger bit, which is bash's rule for the result).
pub(super) struct Expanded {
    pub text: ShortCStr,
    pub origin: Origin,
}

impl Word<'_> {
    /// Expand the word through the ordinary pipeline. A word cut short by the
    /// first-`}` reader (task #174) stays **literal**: expanding the fragment
    /// would turn today's accepted literal output into an `UnclosedParen` or
    /// `UnclosedBacktick` bail. The fd cache is fresh, as
    /// `arith/expand_brace.rs::splice_brace` does it — a `%fd` inside an operator
    /// word does not see the caller's cache (documented, not threaded).
    pub(super) fn expand(&self) -> Result<Expanded, Report<ResolveError>> {
        let bytes = self
            .literal
            .as_bytes()
            .change_context(ResolveError::RefNotFound)?;
        if truncated::truncated(bytes) {
            return Ok(Expanded {
                text: self.literal.clone(),
                origin: Origin::new(),
            });
        }
        let end = self.start + bytes.len();
        let mask = if self.force_unquoted {
            &[]
        } else {
            self.mask.get(self.start..end).unwrap_or(&[])
        };
        let spans = rebased(self.spans, self.start, end);
        let mut cache = HashMap::new();
        let (text, _word_mask, origin) =
            substitute_arg_marked(self.literal, mask, &spans, self.join, &mut cache, self.cell)?;
        Ok(Expanded { text, origin })
    }

    /// The `:?` test for "no word at all": the empty word **without** a zero-byte
    /// quote span at its own start. An unquoted `${u:?""}` has the tokenizer's
    /// span there, so its word counts as a word, expands to nothing, and the
    /// message is empty (bash); the quoted `"${u:?""}"` lost its span to the
    /// parser, so it still reads as no word and keeps the default message —
    /// task #571 owns that span, so the quoted spelling is not a parity pin.
    pub(super) fn is_absent(&self) -> bool {
        self.literal.is_empty() && !self.spans.contains(&self.start)
    }
}

/// Push an expanded word and replay its `$@` boundaries at the outer level,
/// moved by where the word starts in the output. No mask bits are pushed: the
/// caller's `resync` pads the whole range with the outer trigger's quote bit.
pub(super) fn push(expanded: &Expanded, out: &mut ShortCStr, origin: &mut Origin) {
    let before = out.len();
    out.push(&expanded.text);
    origin.append_boundaries(&expanded.origin, before);
}

/// The outer word's spans that belong to this word, in the word's own
/// coordinates. A span at `end` belongs to it — a `~""` at the word's end keeps
/// the tilde literal — and one before `start` belongs to the name or operator.
fn rebased(spans: &[usize], start: usize, end: usize) -> Vec<usize> {
    spans
        .iter()
        .filter(|&&s| start <= s && s <= end)
        .map(|&s| s - start)
        .collect()
}
