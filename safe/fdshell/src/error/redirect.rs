#![forbid(unsafe_code)]

//! Redirection file opening errors (redirect/open.rs, redirect/direction.rs).

use displaydoc::Display;

/// [OpenPath] Failed to open redirection path
#[derive(Display, Debug)]
#[displaydoc("path open failed")]
pub(crate) struct OpenRedirectError;

impl core::error::Error for OpenRedirectError {}
