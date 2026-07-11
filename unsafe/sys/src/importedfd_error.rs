/// Failure to create an `ImportedFd`.
#[derive(displaydoc::Display, Debug)]
pub enum ImportedFdError {
    /// not a valid file descriptor number
    NotANumber,
    /// file descriptor number is negative
    Negative,
    /// fcntl(F_GETFD) failed
    GetFlags,
    /// fcntl(F_SETFD) failed
    SetFlags,
    /// file descriptor is internal (not passed from caller)
    InternalFd,
}

impl core::error::Error for ImportedFdError {}
