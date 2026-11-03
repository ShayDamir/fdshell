//! The **`&>` arm** of the keyword block's tail walk (task #409): a `&>` / `&>>`
//! written after the closing keyword — bare, attached, glued to the closer, in
//! either operator order with another redirect — belongs to the **whole** block,
//! so this module adds the arm and no second reading: the operator, its quoting
//! gate, the `%var` source and the operand it consumes are
//! [`crate::parse::bg_redirect`]'s, the same parser
//! `parse/command/collect_tail.rs` drives for a simple command and
//! `parse/group_block/tail.rs` drives for `{ …; }` — one rule, three readers.
//!
//! What the arm **refuses** is the family's pidvar half (`&>&name`, `&>|&name`):
//! the pidvar is a simple-command position, and bash rejects the spelling behind a
//! closer (rc 2), so the arm reports the family's `&` message at the house rc 1 —
//! the same variant the group's tail already gives every `&` shape. The `%var`
//! target keeps the fd-variable source: that divergence is the family's, documented
//! in POSIX_COMPATIBILITY (">`%var` and `&>%var` redirect to the descriptor"), and
//! it is pinned with its owner, not fixed here.
//!
//! The `wait` block reaches this arm through the shared walk; bash has no `wait`
//! block twin, so that landing is a **decision by sharing**, not parity.

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::bg_redirect::parse_bg_redirect;
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use error_stack::{Report, bail};

/// The family's redirects of the tail token at `i` and the number of following
/// tokens the operator consumes (1 for the bare `&>` / `&>>` that took the next
/// token as its operand, 0 for every attached form) — the contract
/// [`crate::parse::bg_redirect`] gives its callers. `Ok(None)` means the token is
/// not the family's: [`parse_bg_redirect`]'s quoting gate said so (a quoted
/// operator byte is word data, and the walk's own [`super::word`] judge owns that
/// report). A pidvar arm bails: at a closer it is a parse error.
pub(crate) fn defs_at(
    tail: &[Token],
    i: usize,
) -> Result<Option<(Vec<RedirectDef>, usize)>, Report<ParseError>> {
    let Some((bg, extra)) = parse_bg_redirect(tail, i)? else {
        return Ok(None);
    };
    // STYLE §4.15: a negated predicate, so `if` + `bail!` and not `ensure!`.
    if bg.pidvar.is_some() {
        bail!(ParseError::UnexpectedChar { ch: b'&' });
    }
    Ok(Some((bg.redirects, extra)))
}

#[cfg(test)]
mod tests;
