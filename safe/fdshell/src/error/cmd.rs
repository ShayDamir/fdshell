#![forbid(unsafe_code)]

//! Command dispatch errors (run.rs, cond.rs).

use displaydoc::Display;

/// [CmdError] Command dispatch errors
#[derive(Display, Debug)]
pub(crate) enum CmdError {
    /// invalid command
    #[displaydoc("invalid command")]
    Invalid,
    /// builtin failure
    #[displaydoc("builtin failure")]
    Builtin,
}

impl core::error::Error for CmdError {}
