//! `` ` … ` `` command substitution in the argument pipeline: the body runs
//! through the same `cmd_subst` machinery as `$( … )`, so the capture cap, the
//! nesting cap, trailing-newline stripping and the mask rules are inherited.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use super::mask::push_expanded;
use crate::cmd_subst::run_and_capture;
use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::state::ShellState;

/// The caller consumed the opening backtick; the body is scanned, run in a
/// child, and its output is pushed under `quoted` — the trigger byte's mask
/// bit, which is what protects the output from splitting (LESSONS).
pub(super) fn handle(
    peek: &mut super::mask::Counting<'_>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    out_mask: &mut Vec<MaskBit>,
    quoted: bool,
) -> Result<(), Report<ResolveError>> {
    let body = scan_body(peek)?;
    let bytes = body.as_bytes().change_context(ResolveError::RefNotFound)?;
    let output = run_and_capture(bytes, cell).change_context(ResolveError::Resolve)?;
    push_expanded(out, out_mask, &output, quoted)
}

/// Read the body up to the closing backtick, delimiters excluded. An escape
/// pair `\X` is kept verbatim: the backslash shields the closer in this scan
/// and reaches the child, which re-parses the body with its own quote-aware
/// scan — exactly how a `$( … )` body is kept. End of input with no closer is
/// reported as an unclosed backtick; the tokenizer pairs real input before
/// this arm ever runs, so it is a defensive bound for a hand-fed iterator.
fn scan_body(peek: &mut super::mask::Counting<'_>) -> Result<ShortCStr, Report<ResolveError>> {
    let mut body = ShortCStr::new();
    while let Some(b) = peek.next() {
        let escaped = match b {
            b'`' => return Ok(body),
            b'\\' => peek.next(),
            _ => None,
        };
        body.push_byte(b).change_context(ResolveError::NulByte)?;
        if let Some(c) = escaped {
            body.push_byte(c).change_context(ResolveError::NulByte)?;
        }
    }
    bail!(ResolveError::UnclosedBacktick)
}

#[cfg(test)]
mod tests;
