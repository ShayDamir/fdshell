//! Command substitution errors (cmd_subst.rs).

/// [CmdSubstError] Command substitution errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum CmdSubstError {
    /// pipe creation failed
    Pipe,
    /// fork failed
    Fork,
    /// nesting too deep; reduce the depth of nested blocks or command substitutions
    NestingTooDeep,
}

impl core::error::Error for CmdSubstError {}
