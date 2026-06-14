#![forbid(unsafe_code)]

//! Typed errors for the fdshell crate.
//!
//! sys and builtins crates stay as raw `i32` (leaf layers with zero composition).
//! All context is added via `Report::attach()` at each propagation level.

pub(crate) mod capture;
pub(crate) mod cd;
pub(crate) mod cmd;
pub(crate) mod exports;
pub(crate) mod launch;
pub(crate) mod pipeline;
pub(crate) mod redirect;
pub(crate) mod resolve;
pub(crate) mod shell;

use displaydoc::Display;

/// Temporary bridge for un-migrated i32 error sites.
/// Replace with proper typed errors as each domain is migrated.
#[derive(Debug, Display)]
#[displaydoc("syscall error: {0}")]
pub(crate) struct LegacyError(pub(crate) i32);

impl core::error::Error for LegacyError {}
