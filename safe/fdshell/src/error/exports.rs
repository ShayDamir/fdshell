#![forbid(unsafe_code)]

//! export/unset errors (exports.rs).

use displaydoc::Display;

/// [InvalidExportName] Export string contains invalid characters or NUL bytes
#[derive(Display, Debug)]
#[displaydoc("invalid export name")]
pub(crate) struct InvalidExportName;

impl core::error::Error for InvalidExportName {}
