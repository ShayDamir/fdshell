#![forbid(unsafe_code)]

//! File descriptor capture errors (capture.rs).

use displaydoc::Display;

/// [CaptureError] FD capture errors
#[derive(Display, Debug)]
pub(crate) enum CaptureError {
    /// capture target already exists
    #[displaydoc("capture target already exists")]
    Exists,
    /// fd receive failed
    #[displaydoc("fd receive failed")]
    ReceiveFailed,
}

impl core::error::Error for CaptureError {}
