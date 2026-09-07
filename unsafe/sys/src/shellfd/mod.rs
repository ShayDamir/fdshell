use core::sync::atomic::{AtomicBool, Ordering};

pub const TAG_MAX: usize = 4096;

static CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn set_capture_active(active: bool) {
    CAPTURE_ACTIVE.store(active, Ordering::Release);
}

pub fn capture_active() -> bool {
    CAPTURE_ACTIVE.load(Ordering::Acquire)
}

#[repr(C)]
struct CmsgBuf {
    hdr: libc::cmsghdr,
    fd: libc::c_int,
}

mod iovec;

mod send_fd;
pub use send_fd::send_fd;

mod recv_fd;
pub use recv_fd::recv_fd;

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
