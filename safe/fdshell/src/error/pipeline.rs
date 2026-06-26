//! Pipeline execution errors (pipeline/mod.rs).

/// [PipelineError] Pipeline execution errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum PipelineError {
    /// pipe creation failed
    Pipe,
    /// socketpair creation for capture failed
    CaptureSocket,
    /// pipeline execution failed
    Pipeline,
}

impl core::error::Error for PipelineError {}
