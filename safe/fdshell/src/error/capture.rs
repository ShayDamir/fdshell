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
}

impl core::error::Error for CaptureError {}
