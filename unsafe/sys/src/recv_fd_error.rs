//! File descriptor receive errors.

/// Failure to receive a tagged file descriptor over a socket.
#[derive(Debug, displaydoc::Display)]
pub enum RecvFdError {
    /// sender disconnected (zero-length read)
    Closed,
    /// control data truncated by kernel (MSG_CTRUNC)
    CtrlTruncated,
    /// impossible
    Never,
    /// no SCM_RIGHTS fd in message
    NoFd,
    /// sender pid mismatch (got {0}, expected {1})
    PidMismatch(i32, i32),
    /// tag not null-terminated
    TagNotNul,
    /// tag exceeds buffer capacity
    TagTooLong,
}

impl core::error::Error for RecvFdError {}
