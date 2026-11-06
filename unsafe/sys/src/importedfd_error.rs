/// Failure to create an `ImportedFd`.
#[derive(displaydoc::Display, Debug)]
pub enum ImportedFdError {
    /// not a valid file descriptor number
    NotANumber,
    /// file descriptor number is negative
    Negative,
    /// fcntl(F_GETFD) failed
    GetFlags,
    /// fcntl(F_SETFD) failed
    SetFlags,
    /// file descriptor is internal (not passed from caller)
    InternalFd,
    /// impossible error state (should never occur)
    // Truly never: no site in the tree produces it, so no script can print it — the
    // proof is a repo-wide grep, and AGENTS.md's `dead_code` allowance keeps it.
    // The parse paths answer `NotANumber`/`Negative`/`InternalFd` (pinned in
    // `unsafe/sys/tests/importedfd.rs`). Chore #529: excluded from later respawns.
    Never,
}

impl core::error::Error for ImportedFdError {}
