use crate::Fd;

pub const SHELLFD: Fd = 3;
pub const TAG_MAX: usize = 4096;

#[repr(C)]
struct CmsgBuf {
    hdr: libc::cmsghdr,
    fd: libc::c_int,
}

mod send_fd;
pub use send_fd::send_fd;

mod recv_fd;
pub use recv_fd::recv_fd;
