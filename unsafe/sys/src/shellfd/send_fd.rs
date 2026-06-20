use super::CmsgBuf;
use crate::LocalFd;
use crate::iovec::IoVec;
use core::ffi::CStr;

pub fn send_fd(fd: &LocalFd, tag: &CStr) -> Result<(), crate::SyscallError> {
    if !super::capture_active() {
        return Err(crate::SyscallError::ENOENT);
    }
    let tag_bytes = tag.to_bytes_with_nul();
    if tag_bytes.len() > super::TAG_MAX {
        return Err(crate::SyscallError::E2BIG);
    }
    let mut iov = IoVec::new(tag_bytes);
    let mut cmsg = CmsgBuf {
        hdr: libc::cmsghdr {
            // SAFETY: `CMSG_LEN(4)` is a const fn returning 20 on x86_64;
            // the result is stored, not dereferenced.
            cmsg_len: unsafe { libc::CMSG_LEN(core::mem::size_of::<i32>() as u32) as usize },
            cmsg_level: libc::SOL_SOCKET,
            cmsg_type: libc::SCM_RIGHTS,
        },
        fd: fd.as_raw(),
    };
    let msg = libc::msghdr {
        msg_name: core::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: iov.as_mut_ptr(),
        msg_iovlen: 1,
        msg_control: (&raw mut cmsg).cast(),
        msg_controllen: core::mem::size_of_val(&cmsg),
        msg_flags: 0,
    };
    // SAFETY: `iov`, `cmsg`, and `msg` are valid stack-allocated values; `sendmsg`
    // only reads from them. `SHELLFD` must be an open Unix socket.
    crate::cvt(unsafe { libc::sendmsg(super::SHELLFD, &msg, 0) })?;
    Ok(())
}
