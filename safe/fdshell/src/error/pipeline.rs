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
