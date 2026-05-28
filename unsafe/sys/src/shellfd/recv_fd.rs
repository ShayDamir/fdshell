use crate::Fd;
use crate::errno::{EINVAL, EPERM};
use core::ffi::CStr;

pub fn recv_fd<'a>(sock: &Fd, tag: &'a mut [u8], expected_pid: i32) -> Result<(Fd, &'a CStr), i32> {
    let mut extra = 0u8;
    let mut iovs = [
        libc::iovec {
            iov_base: tag.as_mut_ptr() as *mut core::ffi::c_void,
            iov_len: tag.len(),
        },
        libc::iovec {
            iov_base: &mut extra as *mut u8 as *mut core::ffi::c_void,
            iov_len: 1,
        },
    ];
    // SCM_RIGHTS (1 fd: 24 B) + SCM_CREDENTIALS (1 ucred: 32 B) = 56 B
    let mut ctrl_buf = [0u8; 64];
    let mut msg = libc::msghdr {
        msg_name: core::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: iovs.as_mut_ptr(),
        msg_iovlen: 2,
        msg_control: ctrl_buf.as_mut_ptr() as *mut core::ffi::c_void,
        msg_controllen: ctrl_buf.len(),
        msg_flags: 0,
    };
    let n = crate::cvt(unsafe { libc::recvmsg(sock.as_raw(), &mut msg, libc::MSG_CMSG_CLOEXEC) })?
        as usize;

    if msg.msg_flags & libc::MSG_CTRUNC != 0 {
        return Err(EINVAL);
    }

    let mut got_fd: Option<Fd> = None;
    let mut got_pid = None;

    // SAFETY: iterate over control messages in ctrl_buf.
    let cmsg_ptr = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    let mut cmsg = cmsg_ptr;
    while !cmsg.is_null() {
        let level = unsafe { (*cmsg).cmsg_level };
        let ctype = unsafe { (*cmsg).cmsg_type };
        if level == libc::SOL_SOCKET && ctype == libc::SCM_RIGHTS {
            let data = unsafe { libc::CMSG_DATA(cmsg).cast::<i32>() };
            let nbytes = (unsafe { (*cmsg).cmsg_len } as usize)
                .saturating_sub(core::mem::size_of::<libc::cmsghdr>());
            let nfds = nbytes / core::mem::size_of::<i32>();
            for i in 0..nfds {
                let raw_fd = unsafe { *data.add(i) };
                if got_fd.is_none() {
                    got_fd = Some(unsafe { Fd::from_raw(raw_fd) });
                } else {
                    unsafe { libc::close(raw_fd) };
                }
            }
        } else if level == libc::SOL_SOCKET && ctype == libc::SCM_CREDENTIALS {
            let cred = unsafe { &*libc::CMSG_DATA(cmsg).cast::<libc::ucred>() };
            got_pid = Some(cred.pid);
        }
        cmsg = unsafe { libc::CMSG_NXTHDR(&msg, cmsg) };
    }

    let fd = got_fd.ok_or(EINVAL)?;
    if let Some(pid) = got_pid
        && pid != expected_pid
    {
        return Err(EPERM);
    }

    if n > tag.len() {
        return Err(EINVAL);
    }
    let tag_slice = tag.get(..n).ok_or(EINVAL)?;
    let tag_cstr = CStr::from_bytes_with_nul(tag_slice).map_err(|_| EINVAL)?;
    Ok((fd, tag_cstr))
}
