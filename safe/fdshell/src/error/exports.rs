#![forbid(unsafe_code)]

//! export/unset errors (exports.rs).

use displaydoc::Display;

/// [InvalidExportName] Export string contains NUL bytes or internal inconsistency
#[derive(Display, Debug)]
pub(crate) struct InvalidExportName;

impl core::error::Error for InvalidExportName {}
