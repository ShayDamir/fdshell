#![forbid(unsafe_code)]

//! Command substitution errors (cmd_subst.rs).

use displaydoc::Display;

/// [CmdSubstError] Command substitution errors
#[derive(Display, Debug)]
pub(crate) enum CmdSubstError {
    /// pipe creation failed
    Pipe,
    /// fork failed
    Fork,
}

impl core::error::Error for CmdSubstError {}
