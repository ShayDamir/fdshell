use core::sync::atomic::{AtomicBool, Ordering};

use crate::LocalFd;

pub const SHELLFD: i32 = 3;
pub const SHELLFD_STR: &core::ffi::CStr = c"3";
pub const TAG_MAX: usize = 4096;

static CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn set_capture_active(active: bool) {
    CAPTURE_ACTIVE.store(active, Ordering::Release);
}

pub fn capture_active() -> bool {
    CAPTURE_ACTIVE.load(Ordering::Acquire)
}

/// Reserve SHELLFD by placing a harmless pipe read-end there,
/// preventing subsequent `socketpair()` from returning it.
pub fn reserve_shellfd() -> Result<LocalFd, crate::SyscallError> {
    let (rd, _wr) = crate::pipe::pipe2(libc::O_CLOEXEC)?;
    if rd.as_raw() != SHELLFD {
        rd.try_clone_to(SHELLFD)
    } else {
        Ok(rd)
    }
}

#[repr(C)]
struct CmsgBuf {
    hdr: libc::cmsghdr,
    fd: libc::c_int,
}

mod send_fd;
pub use send_fd::send_fd;

mod recv_fd;
pub use recv_fd::recv_fd;
