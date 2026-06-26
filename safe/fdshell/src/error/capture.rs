//! File descriptor capture errors (capture.rs).

/// [CaptureError] FD capture errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum CaptureError {
    /// capture target already exists
    Exists,
    /// fd receive failed
    ReceiveFailed,
}

impl core::error::Error for CaptureError {}
