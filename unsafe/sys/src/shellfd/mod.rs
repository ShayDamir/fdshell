use crate::DupFd;

pub const SHELLFD: i32 = 3;
pub const SHELL_DUPFD: DupFd = unsafe { DupFd::from_raw(SHELLFD) };
pub const TAG_MAX: usize = 4096;

/// Reserve SHELLFD by placing a harmless pipe read-end there,
/// preventing subsequent `socketpair()` from returning it.
pub fn reserve_shellfd() -> Result<(), i32> {
    let (rd, wr) = crate::pipe::pipe2(libc::O_CLOEXEC)?;
    if rd.as_raw() != SHELLFD {
        rd.dup2(SHELL_DUPFD)?;
        rd.close()?;
    } else {
        core::mem::forget(rd);
    }
    wr.close()?;
    Ok(())
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
