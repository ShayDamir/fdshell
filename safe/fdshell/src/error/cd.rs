//! cd command errors (cd.rs).

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
    // Truly never: no site in the tree produces it, so no script can print it — the
    // proof is a repo-wide grep, and AGENTS.md's `dead_code` allowance keeps it.
    // Chore #529: excluded from later respawns.
    Never,
}

impl core::error::Error for CdError {}
