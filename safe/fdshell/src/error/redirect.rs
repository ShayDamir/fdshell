//! Redirection file opening errors (redirect/open.rs, redirect/direction.rs).

/// [OpenRedirectError] Failed to open redirection path
#[derive(displaydoc::Display, Debug)]
pub(crate) struct OpenRedirectError;

impl core::error::Error for OpenRedirectError {}
