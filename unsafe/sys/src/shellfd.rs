use core::ffi::CStr;
use crate::Fd;

pub const SHELLFD: Fd = 3;
pub const TAG_MAX: usize = 4096;

#[repr(C)]
struct CmsgBuf {
    hdr: libc::cmsghdr,
    fd: libc::c_int,
}

pub fn send_fd(fd: Fd, tag: &CStr) -> Result<(), i32> {
    let tag_bytes = tag.to_bytes_with_nul();
    if tag_bytes.len() > TAG_MAX {
        return Err(libc::E2BIG);
    }
    let iov = libc::iovec {
        iov_base: tag_bytes.as_ptr() as *mut core::ffi::c_void,
        iov_len: tag_bytes.len(),
    };
    let mut cmsg = CmsgBuf {
        // SAFETY: `CMSG_LEN` is a const fn in libc; passing `4` (size of one `i32`)
        // is always valid and returns `20` on x86_64.
        hdr: libc::cmsghdr {
            cmsg_len: unsafe { libc::CMSG_LEN(4) as usize },
            cmsg_level: libc::SOL_SOCKET,
            cmsg_type: libc::SCM_RIGHTS,
        },
        fd,
    };
    let msg = libc::msghdr {
        msg_name: core::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: &iov as *const libc::iovec as *mut libc::iovec,
        msg_iovlen: 1,
        msg_control: &mut cmsg as *mut CmsgBuf as *mut core::ffi::c_void,
        msg_controllen: core::mem::size_of::<CmsgBuf>(),
        msg_flags: 0,
    };
    // SAFETY: `iov`, `cmsg`, and `msg` are valid stack-allocated values; `sendmsg`
    // only reads from them. `SHELLFD` must be an open Unix socket.
    crate::cvt(unsafe { libc::sendmsg(SHELLFD, &msg, 0) })?;
    Ok(())
}

pub fn recv_fd(sock: Fd, tag: &mut [u8]) -> Result<Fd, i32> {
    let mut iov = libc::iovec {
        iov_base: tag.as_mut_ptr() as *mut core::ffi::c_void,
        iov_len: tag.len(),
    };
    // SAFETY: `CmsgBuf` contains only integer types (`usize`, `i32`) for which the
    // all-zero bit pattern is valid. The struct is `#[repr(C)]`.
    let mut cmsg: CmsgBuf = unsafe { core::mem::zeroed() };
    let mut msg = libc::msghdr {
        msg_name: core::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: &mut iov as *mut libc::iovec,
        msg_iovlen: 1,
        msg_control: &mut cmsg as *mut CmsgBuf as *mut core::ffi::c_void,
        msg_controllen: core::mem::size_of::<CmsgBuf>(),
        msg_flags: 0,
    };
    // SAFETY: `iov`, `cmsg`, `msg` are valid stack-allocated values; `recvmsg` writes
    // into `tag` and `cmsg` within their allocated sizes. `sock` must be an open socket.
    crate::cvt(unsafe { libc::recvmsg(sock, &mut msg, libc::MSG_CMSG_CLOEXEC) })?;
    // SAFETY: `CMSG_FIRSTHDR` dereferences `msg` which is a valid local; returns null
    // if no control message is present (handled below).
    let cmsg_ptr = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    if cmsg_ptr.is_null() {
        return Err(libc::EINVAL);
    }
    // SAFETY: `cmsg_ptr` is non-null, points into our `cmsg` buffer.  `CMSG_DATA` computes
    // the offset past the `cmsghdr` header; on x86_64 this is 16 bytes, which is within
    // the `CmsgBuf` allocation. The cast to `*const i32` has alignment 4 ≤ 8.
    unsafe {
        if (*cmsg_ptr).cmsg_level != libc::SOL_SOCKET
            || (*cmsg_ptr).cmsg_type != libc::SCM_RIGHTS
        {
            return Err(libc::EINVAL);
        }
        Ok(*libc::CMSG_DATA(cmsg_ptr).cast::<i32>())
    }
}
