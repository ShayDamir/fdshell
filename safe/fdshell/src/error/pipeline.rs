#![forbid(unsafe_code)]

//! Pipeline execution errors (pipeline/mod.rs).

use displaydoc::Display;

/// [PipelineError] Pipeline execution errors
#[derive(Display, Debug)]
pub(crate) enum PipelineError {
    /// pipe creation failed
    #[displaydoc("pipe creation failed")]
    Pipe,
    /// socketpair creation for capture failed
    #[displaydoc("capture socketpair creation failed")]
    CaptureSocket,
    /// pipeline execution failed
    #[displaydoc("pipeline failed")]
    Pipeline,
}

impl core::error::Error for PipelineError {}
