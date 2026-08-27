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
    /// command substitution output exceeds the capture limit; produce less output or write to a file
    OutputTooLarge,
    /// impossible error state (should never occur)
    Never,
}

impl core::error::Error for CmdSubstError {}
