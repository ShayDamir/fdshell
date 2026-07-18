/// Failure to verify a `LocalFd`.
#[derive(displaydoc::Display, Debug)]
pub enum LocalFdError {
    /// fcntl(F_GETFD) failed
    GetFlags,
    /// file descriptor does not have CLOEXEC set (ownership invariant broken)
    NoCloexec,
}

impl core::error::Error for LocalFdError {}
