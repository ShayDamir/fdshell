#![forbid(unsafe_code)]

//! Pipeline execution errors (pipeline/mod.rs).

use displaydoc::Display;

/// [PipelineError] Pipeline execution errors
#[derive(Display, Debug)]
pub(crate) enum PipelineError {
    /// pipe creation failed
    Pipe,
    /// socketpair creation for capture failed
    CaptureSocket,
    /// pipeline execution failed
    Pipeline,
}

impl core::error::Error for PipelineError {}

impl From<crate::error::capture::CaptureError> for PipelineError {
    fn from(_: crate::error::capture::CaptureError) -> Self {
        PipelineError::Pipeline
    }
}

impl From<crate::error::launch::LaunchError> for PipelineError {
    fn from(_: crate::error::launch::LaunchError) -> Self {
        PipelineError::Pipeline
    }
}
