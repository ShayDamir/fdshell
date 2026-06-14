#![forbid(unsafe_code)]

//! cd command errors (cd/mod.rs).

use displaydoc::Display;

/// [CdError] Directory change errors
#[derive(Display, Debug)]
pub(crate) enum CdError {
    /// $HOME not set
    #[displaydoc("$HOME not set")]
    HomeNotSet,
    /// cd path open failed
    #[displaydoc("cd path open failed")]
    CdPathOpen,
    /// $OLDCWD not set
    #[displaydoc("$OLDCWD not set")]
    OldcwdNotSet,
}

impl core::error::Error for CdError {}
