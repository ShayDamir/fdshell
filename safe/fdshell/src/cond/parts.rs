//! The cond list's *parts*: how a part gets its share of the heredoc bodies,
//! when a part is empty, how the rest of a failed `&&` chain is skipped up
//! to the next `||`, and when an operator has no command on one of its two
//! sides — behind it ([`dangling_operator`]) or in front of it
//! ([`empty_left_side`]). Body allocation follows the global per-line operator
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

/// The first operator whose **left** side is blank **and** whose right side is a
/// command, walked over a text that begins at a physical-line start
/// (`at_line_start`) — the operator bash's `command (AND-IF-TERMINATOR command)`
/// grammar cannot serve, because it needs a command on **both** sides and this one
/// has none in front of it (task #406, the mirror half of [`dangling_operator`]).
/// The walk is the shared boundary scan, so quotes, escapes, comments, regions and
/// the glued pair are read by the existing rules and never by a byte compare. A
/// text that begins **mid-line** is not this rule's: its head was claimed by
/// another layer (a definition's body, a here-doc region), so the operator does
/// have its command — in front of it, on the line this text is the tail of. A text
/// that is **only** an operator has a blank side on both, and each rule declines
/// it: the shape stays the deferral's, and its rows are pinned at bash's measured
/// value. The operator's name is the report's parameter, and the caller reports
/// **before** any part of the list runs.
pub(crate) fn empty_left_side(line: &[u8], at_line_start: bool) -> Option<ShortCStr> {
    if !at_line_start {
        return None;
    }
    let mut state = ScanState::new();
    let mut start = 0;
    loop {
        let (i, _is_or) = boundary::next_boundary(line, start, &mut state);
        if i == line.len() {
            return None;
        }
        if is_empty_part(line, start, i) && !is_empty_part(line, i + 2, line.len()) {
            return op_name(line, i);
        }
        start = i + 2;
    }
}

/// The name of the top-level cond operator at `i` — the report's parameter, spelled
/// by the scanner's own literals (STYLE §6.3). Both halves of the both-sides rule
/// answer with it.
fn op_name(line: &[u8], at: usize) -> Option<ShortCStr> {
    match line.get(at).copied() {
        Some(b'&') => Some(ShortCStr::from(c"&&")),
        Some(b'|') => Some(ShortCStr::from(c"||")),
        _ => None,
    }
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
    // needs a command on **both** sides: the empty *right* side is this rule, the
    // empty *left* side — an operator that opens the text it is handed — is
    // [`empty_left_side`] (task #406). A blank left side is the split this rule
    // declines to report, so the two together are the one grammar sentence.
    let before = line
        .get(..start)
        .is_none_or(|pre| pre.trim_ascii().is_empty());
    if before {
        return None;
    }
    op_name(line, start)
}
