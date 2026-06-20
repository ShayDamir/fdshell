#![forbid(unsafe_code)]

//! export/unset errors (exports.rs).

use displaydoc::Display;

/// [InvalidExportName] Export string contains invalid characters or NUL bytes
#[derive(Display, Debug)]
pub(crate) struct InvalidExportName;

impl core::error::Error for InvalidExportName {}

impl From<i32> for InvalidExportName {
    fn from(_: i32) -> Self {
        InvalidExportName
    }
}
