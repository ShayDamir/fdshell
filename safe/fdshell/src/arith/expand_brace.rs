//! The `${…}` arm of the arithmetic pre-expansion pass: a closed region is
//! rebuilt as the tokenizer's word and handed to the word path's non-split
//! substitution entry (the value is stored whole, so no field splitting —
//! bash expands first, then parses the resulting string).

use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::region_scan::scan_brace;
use crate::state::ShellState;
use crate::substitute::substitute_arg;

/// Splice the `${…}` region whose `$` sits at `dollar_at` into `out`, returning
/// the index just past the region. An unclosed region stays literal text
/// (byte-at-a-time fallback), reaching the lexer as a syntax error as before.
pub(crate) fn splice_brace(
    body: &[u8],
    dollar_at: usize,
    cell: &ForkCell<ShellState>,
    out: &mut Vec<u8>,
) -> Result<usize, Report<ResolveError>> {
    let (inner, consumed, closed) = scan_brace(body, dollar_at);
    if !closed {
        out.push(b'$');
        return Ok(dollar_at + 1);
    }
    let word = brace_word(&inner)?;
    // The mask travels with the word (LESSONS): the region stood in unquoted
    // body bytes, so the derived mask is all-unquoted; stripped quote markers
    // own no bit (task #321), which is the honest reading. Spans are
    // input-side only and inert on a `$`-initial word.
    let mask = vec![MaskBit::Unquoted; word.len()];
    let (value, _) = substitute_arg(&word, &mask, &[], &mut HashMap::new(), cell)?;
    out.extend_from_slice(value.as_bytes().change_context(ResolveError::Never)?);
    Ok(dollar_at + consumed)
}

/// Rebuild a closed `${…}` region as the tokenizer would hand it to
/// substitution: `$`, `{`, the quote-stripped body (a quote is a marker, not
/// content), `}`.
fn brace_word(inner: &[u8]) -> Result<ShortCStr, Report<ResolveError>> {
    let mut word = ShortCStr::new();
    // The three literals hold no NUL, so their pushes cannot fail (§4.10).
    word.push_checked(b"${")
        .change_context(ResolveError::Never)?;
    word.push_checked(inner)
        .change_context(ResolveError::NulByte)?;
    word.push_checked(b"}")
        .change_context(ResolveError::Never)?;
    Ok(word)
}
