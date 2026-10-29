//! The end-of-file here-doc: the operators of a line whose delimiter line never
//! arrived, because the text ended first (task #364).
//!
//! The parse layer owns **both** halves of bash's behavior here: it accepts the
//! body the stream leaves — the remaining lines, or nothing when none are left —
//! and it prints the end-of-file warning. The allocator claims the same span (one
//! entry per operator, LESSONS) but never decides, and it does not re-implement
//! this rule: its region carries no synthesized delimiter line, so the text the
//! rebuild hands back still reads as an end-of-file here-doc and the decision stays
//! with this single owner.

use crate::scan::heredoc::{Operator, Span};
use crate::warn;
use alloc::vec::Vec;

/// Print bash's end-of-file warning once per operator the parse layer decides for
/// — including the operators behind the first, which reach it as claimed-but-never-
/// found regions. A statement that is never parsed (an arm that does not run) never
/// warns: bash parses the whole script and warns for it anyway, which is the
/// documented divergence.
pub(crate) fn warn_eof(spans: &[Span], ops: &[Operator]) {
    for (span, op) in spans.iter().zip(ops) {
        if !span.found {
            warn::here_doc_eof(&op.delim);
        }
    }
}

/// Close an end-of-file body: bash reads the body as whole lines, so the line the
/// text ends on gets the newline it lacks (`cat <<E` + `bodyE` prints `bodyE` on a
/// line of its own). An empty body — nothing was left — stays empty, and a span
/// whose delimiter line was found keeps its bytes byte-exact.
pub(crate) fn close_last_line(bytes: Vec<u8>) -> Vec<u8> {
    let mut bytes = bytes;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes
}
