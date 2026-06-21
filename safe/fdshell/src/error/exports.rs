#![forbid(unsafe_code)]

//! export/unset errors (exports.rs).

use displaydoc::Display;

/// [ExportError] Export errors
#[derive(Display, Debug)]
pub(crate) enum ExportError {
    /// NUL byte in export string
    NulByte,
    /// internal inconsistency in export data
    InternalInconsistency,
}

impl core::error::Error for ExportError {}
