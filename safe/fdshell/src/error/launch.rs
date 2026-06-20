#![forbid(unsafe_code)]

//! Launch / child execution errors (launch.rs, child/*.rs).

use displaydoc::Display;

/// [LaunchError] Launch / child execution errors
#[derive(Display, Debug)]
pub(crate) enum LaunchError {
    /// fork syscall failed
    Fork,
    /// child exec syscall failed
    Exec,
    /// builtin dispatch in child failed
    BuiltinDispatch,
    /// redirect file open failed
    Redirect,
    /// capture socketpair creation failed
    CaptureSocket,
    /// state borrow failed
    Borrow,
}

impl core::error::Error for LaunchError {}

impl From<crate::error::redirect::OpenRedirectError> for LaunchError {
    fn from(_: crate::error::redirect::OpenRedirectError) -> Self {
        LaunchError::Redirect
    }
}

impl From<crate::error::capture::CaptureError> for LaunchError {
    fn from(_: crate::error::capture::CaptureError) -> Self {
        LaunchError::CaptureSocket
    }
}
