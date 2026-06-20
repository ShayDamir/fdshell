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
    /// {command}: captures are not supported
    CapturesNotSupported { command: &'static str },
    /// {command}: redirects are not supported
    RedirectNotSupported { command: &'static str },
    /// {command}: `builtin` prefix is not supported
    BuiltinKeywordNotSupported { command: &'static str },
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
    /// cd failed
    Cd,
    /// invalid export name
    ExportName,
    /// fd pass-through failed
    FdPass,
    /// command substitution failed
    CmdSubst,
    /// task management failed
    Task,
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

impl From<crate::error::cd::CdError> for CmdError {
    fn from(_: crate::error::cd::CdError) -> Self {
        CmdError::Cd
    }
}

impl From<crate::error::exports::InvalidExportName> for CmdError {
    fn from(_: crate::error::exports::InvalidExportName) -> Self {
        CmdError::ExportName
    }
}

impl From<crate::error::fdpass::FdPassError> for CmdError {
    fn from(_: crate::error::fdpass::FdPassError) -> Self {
        CmdError::FdPass
    }
}

impl From<crate::error::cmd_subst::CmdSubstError> for CmdError {
    fn from(_: crate::error::cmd_subst::CmdSubstError) -> Self {
        CmdError::CmdSubst
    }
}

impl From<crate::error::task::TaskError> for CmdError {
    fn from(_: crate::error::task::TaskError) -> Self {
        CmdError::Task
    }
}

impl From<crate::error::cd::CdError> for Report<CmdError> {
    fn from(err: crate::error::cd::CdError) -> Self {
        Report::new(err).change_context(CmdError::Cd)
    }
}

impl From<crate::error::exports::InvalidExportName> for Report<CmdError> {
    fn from(err: crate::error::exports::InvalidExportName) -> Self {
        Report::new(err).change_context(CmdError::ExportName)
    }
}

impl From<crate::error::fdpass::FdPassError> for Report<CmdError> {
    fn from(err: crate::error::fdpass::FdPassError) -> Self {
        Report::new(err).change_context(CmdError::FdPass)
    }
}

impl From<crate::error::cmd_subst::CmdSubstError> for Report<CmdError> {
    fn from(err: crate::error::cmd_subst::CmdSubstError) -> Self {
        Report::new(err).change_context(CmdError::CmdSubst)
    }
}

impl From<crate::error::task::TaskError> for Report<CmdError> {
    fn from(err: crate::error::task::TaskError) -> Self {
        Report::new(err).change_context(CmdError::Task)
    }
}
