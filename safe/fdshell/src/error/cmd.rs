#![forbid(unsafe_code)]

//! Command dispatch errors (run.rs, cond.rs).

use displaydoc::Display;
use error_stack::Report;

/// [CmdError] Command dispatch errors
#[derive(Display, Debug)]
pub(crate) enum CmdError {
    /// invalid command
    Invalid,
    /// builtin failure
    Builtin,
    /// parse error
    Parse,
    /// launch failed
    Launch,
    /// capture failed
    Capture,
    /// pipeline failed
    Pipeline,
    /// redirection failed
    Redirect,
    /// resolution error
    Resolve,
    /// execution error
    Exec,
}

impl core::error::Error for CmdError {}

impl From<crate::error::capture::CaptureError> for CmdError {
    fn from(_: crate::error::capture::CaptureError) -> Self {
        CmdError::Capture
    }
}

impl From<crate::error::redirect::OpenRedirectError> for CmdError {
    fn from(_: crate::error::redirect::OpenRedirectError) -> Self {
        CmdError::Redirect
    }
}

impl From<crate::error::launch::LaunchError> for CmdError {
    fn from(_: crate::error::launch::LaunchError) -> Self {
        CmdError::Launch
    }
}

impl From<crate::error::pipeline::PipelineError> for CmdError {
    fn from(_: crate::error::pipeline::PipelineError) -> Self {
        CmdError::Pipeline
    }
}

impl From<crate::error::launch::LaunchError> for Report<CmdError> {
    fn from(err: crate::error::launch::LaunchError) -> Self {
        Report::new(err).change_context(CmdError::Launch)
    }
}

impl From<crate::error::pipeline::PipelineError> for Report<CmdError> {
    fn from(err: crate::error::pipeline::PipelineError) -> Self {
        Report::new(err).change_context(CmdError::Pipeline)
    }
}
