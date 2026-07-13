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

mod send_fd;
pub use send_fd::send_fd;

mod recv_fd;
pub use crate::RecvFdError;
pub use recv_fd::recv_fd;
