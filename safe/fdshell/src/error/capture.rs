//! File descriptor capture errors (capture.rs).

/// [CaptureError] FD capture errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum CaptureError {
    /// capture target already exists
    Exists,
    /// fd receive failed
    ReceiveFailed,
    /// incomplete capture — expected {expected} but received {received}
    // §4.2 prefers plain variants; counts required for actionable message (§4.7).
    Incomplete { expected: usize, received: usize },
    /// internal invariant violated
    // Truly never: the only producer is `capture/slot.rs`'s `finish`, which
    // `do_captures` runs only after every slot passes `satisfied()`, and a slot with
    // no cap satisfies only through a non-empty `entries`. Chore #516: excluded from
    // later respawns.
    Never,
}

impl core::error::Error for CaptureError {}
