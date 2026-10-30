//! The cond list's *parts*: how a part gets its share of the heredoc bodies,
//! when a part is empty, how the rest of a failed `&&` chain is skipped up
//! to the next `||`, and when the list ends on an operator with no command
//! behind it. Body allocation follows the global per-line operator
//! order (LESSONS.md: "A heredoc body is a property of the logical line, not of
//! a contiguous span"); the boundary scanning itself is `boundary`.

use super::boundary;
use crate::scan::{ScanState, heredoc, last_op};
use alloc::vec::Vec;
use sys::ShortCStr;

/// After a failing `&&` part, the next `||` (or end of line): everything up
/// to it is skipped without running. The skipped parts' body bytes are still
/// consumed (the bodies are in global operator order).
pub(crate) fn skip_to_or(
    line: &[u8],
    from: usize,
    state: &mut ScanState,
    bodies: &[Vec<u8>],
    body_idx: &mut usize,
) -> usize {
    let mut pos = from;
    loop {
        let (p, is_or) = boundary::next_boundary(line, pos, state);
        take_bodies(line, pos, p, bodies, body_idx);
        if is_or || p == line.len() {
            return p;
        }
        pos = p + 2;
    }
}

/// The next `n` body bytes for the part `line[start..end]`, where `n` is the
/// part's byte-level `<<` operator count. The bodies are in global operator
/// order, so the part's share is the next `n` entries. The count is the
/// **window-blind** one: a part's text is a block/arm text (`cond/part.rs::reconstruct`)
/// and spans body and terminator lines, whose bytes are heredoc structure and
/// never the part's operators.
pub(crate) fn take_bodies(
    line: &[u8],
    start: usize,
    end: usize,
    bodies: &[Vec<u8>],
    body_idx: &mut usize,
) -> Vec<Vec<u8>> {
    let n = heredoc::operator_count_opaque(line, start, end);
    let mut out = Vec::new();
    for _ in 0..n {
        if let Some(body) = bodies.get(*body_idx) {
            out.push(body.clone());
            *body_idx += 1;
        }
    }
    out
}

pub(crate) fn is_empty_part(line: &[u8], start: usize, i: usize) -> bool {
    line.get(start..i).unwrap_or(b"").trim_ascii().is_empty()
}

/// The `&&` / `||` that is the text's **last token** — nothing but whitespace,
/// comment text and here-doc regions sits behind it, so the operator has no
/// command after it and bash's grammar refuses the list (task #367). The walk is
/// the shared state-aware one (task #492): quotes, escapes, comments, regions,
/// `$( )` and the glued pair are read by the existing scan rules, never by a
/// byte compare, and a list whose command lives behind a physical-line end has
/// already been joined onto the line, so it arrives here complete. The operator's
/// name is the report's parameter, and the caller reports **before** any part of
/// the list runs, as bash runs nothing of a list it refuses.
pub(crate) fn dangling_operator(line: &[u8]) -> Option<ShortCStr> {
    let (start, _len) = last_op::walk(line, line.len() + 1).1?;
    // Bash's and-list is `command (AND-IF-TERMINATOR command) …`, so an operator
    // needs a command on **both** sides. One that opens the text — an operator
    // alone below a `<<` line, `x\n&& y` — is the empty-*left* family with its own
    // owner (task #406); only the empty *right* side is this rule's.
    let before = line
        .get(..start)
        .is_none_or(|pre| pre.trim_ascii().is_empty());
    if before {
        return None;
    }
    // The walk reports only `&&` and `||`; its predicate's own byte tells them
    // apart, and the literals are the scanner's, not user data (STYLE §6.3).
    match line.get(start).copied() {
        Some(b'&') => Some(ShortCStr::from(c"&&")),
        Some(b'|') => Some(ShortCStr::from(c"||")),
        _ => None,
    }
}
