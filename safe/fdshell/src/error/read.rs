//! read builtin errors.

/// [ReadError] read builtin errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum ReadError {
    /// expected variable name
    NoTarget,
    /// invalid variable name
    BadTarget,
    /// missing argument for -{0} flag
    MissingArgument(char),
    /// invalid argument for -{0} flag
    InvalidArgument(char),
    /// variable not found
    VarNotFound,
    /// I/O error
    Io,
    /// input contains NUL byte
    NulByte,
    /// fd variable target not yet supported
    FdVarUnsupported,
}

impl core::error::Error for ReadError {}
