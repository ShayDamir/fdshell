#![forbid(unsafe_code)]

//! cd command errors (cd/mod.rs).

use displaydoc::Display;

/// [CdError] Directory change errors
#[derive(Display, Debug)]
pub(crate) enum CdError {
    /// $HOME not set
    HomeNotSet,
    /// cd path open failed
    CdPathOpen,
    /// $OLDCWD not set
    OldcwdNotSet,
}

impl core::error::Error for CdError {}
