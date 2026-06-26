//! Command substitution errors (cmd_subst.rs).

/// [CmdSubstError] Command substitution errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum CmdSubstError {
    /// pipe creation failed
    Pipe,
    /// fork failed
    Fork,
}

impl core::error::Error for CmdSubstError {}
