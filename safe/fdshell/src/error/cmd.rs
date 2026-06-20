#![forbid(unsafe_code)]

//! Command dispatch errors (run.rs, cond.rs).

use displaydoc::Display;

/// [CmdError] Command dispatch errors
#[derive(Display, Debug)]
pub(crate) enum CmdError {
    /// invalid command
    Invalid,
    /// builtin failure
    Builtin,
    /// parse error
    Parse,
    /// execution error
    Exec,
    /// resolution error
    Resolve,
}

impl core::error::Error for CmdError {}
