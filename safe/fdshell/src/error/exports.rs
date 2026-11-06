//! export/unset errors (exports.rs).

/// [ExportError] Export errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum ExportError {
    /// NUL byte in export string
    NulByte,
    /// internal inconsistency in export data
    InternalInconsistency,
    /// impossible — internal state is valid
    // Truly never: no site in the tree produces it, so no script can print it — the
    // proof is a repo-wide grep, and AGENTS.md's `dead_code` allowance keeps it.
    // Its live variants (`NulByte`, `Io`) are the domain answers.
    // Chore #529: excluded from later respawns.
    Never,
    /// I/O error
    Io,
}

impl core::error::Error for ExportError {}
