use super::CmsgBuf;
use crate::Fd;
use crate::errno::E2BIG;
use core::ffi::CStr;

pub fn send_fd(fd: &Fd, tag: &CStr) -> Result<(), i32> {
    if !super::capture_active() {
        return Err(crate::errno::ENOENT);
    }
    let tag_bytes = tag.to_bytes_with_nul();
    if tag_bytes.len() > super::TAG_MAX {
        return Err(E2BIG);
    }
    let iov = libc::iovec {
        iov_base: tag_bytes.as_ptr() as *mut core::ffi::c_void,
        iov_len: tag_bytes.len(),
    };
    let mut cmsg = CmsgBuf {
        // SAFETY: `CMSG_LEN` is a const fn in libc; passing `4` (size of one `i32`)
        // is always valid and returns `20` on x86_64.
        hdr: libc::cmsghdr {
            cmsg_len: unsafe { libc::CMSG_LEN(core::mem::size_of::<i32>() as u32) as usize },
            cmsg_level: libc::SOL_SOCKET,
            cmsg_type: libc::SCM_RIGHTS,
        },
        fd: fd.as_raw(),
    };
    let msg = libc::msghdr {
        msg_name: core::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: &iov as *const libc::iovec as *mut libc::iovec,
        msg_iovlen: 1,
        msg_control: &mut cmsg as *mut CmsgBuf as *mut core::ffi::c_void,
        msg_controllen: core::mem::size_of_val(&cmsg),
        msg_flags: 0,
    };
    // SAFETY: `iov`, `cmsg`, and `msg` are valid stack-allocated values; `sendmsg`
    // only reads from them. `SHELLFD` must be an open Unix socket.
    crate::cvt(unsafe { libc::sendmsg(super::SHELLFD, &msg, 0) })?;
    Ok(())
}
