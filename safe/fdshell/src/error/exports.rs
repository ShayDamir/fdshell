//! export/unset errors (exports.rs).

/// [ExportError] Export errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum ExportError {
    /// NUL byte in export string
    NulByte,
    /// internal inconsistency in export data
    InternalInconsistency,
    /// impossible — internal state is valid
    Never,
    /// I/O error
    Io,
}

impl core::error::Error for ExportError {}
