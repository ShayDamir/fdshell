//! cd command errors (cd/mod.rs).

/// [CdError] Directory change errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum CdError {
    /// $HOME not set
    HomeNotSet,
    /// cd path open failed
    CdPathOpen,
    /// $OLDCWD not set
    OldcwdNotSet,
    /// fd variable not set
    FdNotSet,
    /// impossible
    Never,
}

impl core::error::Error for CdError {}
