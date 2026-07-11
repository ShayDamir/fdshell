use error_stack::{Report, ResultExt, bail, ensure};

use crate::LocalFd;
use crate::iovec::IoVecMut;
use core::ffi::CStr;

pub fn recv_fd<'a>(
    sock: &LocalFd,
    tag: &'a mut [u8],
    expected_pid: i32,
) -> Result<(LocalFd, &'a CStr), Report<crate::RecvFdError>> {
    let mut extra = [0u8; 1];
    // SCM_RIGHTS (1 fd: 24 B) + SCM_CREDENTIALS (1 ucred: 32 B) = 56 B
    let mut ctrl_buf = [0u8; 64];
    let mut iovs = [IoVecMut::new(tag), IoVecMut::new(&mut extra)];
    let mut msg = libc::msghdr {
        msg_name: core::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: iovs.as_mut_ptr().cast(),
        msg_iovlen: 2,
        msg_control: ctrl_buf.as_mut_ptr().cast(),
        msg_controllen: ctrl_buf.len(),
        msg_flags: 0,
    };
    // SAFETY: `sock` is a valid open socket; `msg` and `ctrl_buf`
    // are valid stack allocations; `recvmsg` with invalid pointers
    // returns -1/EFAULT, caught by `cvt`.
    let n = crate::cvt(unsafe { libc::recvmsg(sock.as_raw(), &mut msg, libc::MSG_CMSG_CLOEXEC) })
        .change_context(crate::RecvFdError::Closed)? as usize;

    ensure!(n > 0, crate::RecvFdError::Closed);
    ensure!(
        msg.msg_flags & libc::MSG_CTRUNC == 0,
        crate::RecvFdError::CtrlTruncated
    );

    let mut got_fd: Option<LocalFd> = None;
    let mut got_pid = None;

    // SAFETY: `CMSG_FIRSTHDR` returns a pointer into `ctrl_buf`
    // (valid allocation), or null if no messages.
    let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    while !cmsg.is_null() {
        // SAFETY: `cmsg` is non-null, returned by `CMSG_FIRSTHDR`/
        // `CMSG_NXTHDR`; the pointer is valid for a `cmsghdr`.
        let level = unsafe { (*cmsg).cmsg_level };
        // SAFETY: same pointer validity as above.
        let ctype = unsafe { (*cmsg).cmsg_type };
        if level == libc::SOL_SOCKET && ctype == libc::SCM_RIGHTS {
            // SAFETY: `cmsg` is a valid `cmsghdr` pointer; `CMSG_DATA`
            // is valid for `cmsg_len` bytes.
            let data = unsafe { libc::CMSG_DATA(cmsg).cast::<i32>() };
            // SAFETY: `cmsg` is valid (same as above).
            let nfds = ((unsafe { (*cmsg).cmsg_len } as usize)
                .saturating_sub(core::mem::size_of::<libc::cmsghdr>()))
                / core::mem::size_of::<i32>();
            for i in 0..nfds {
                // SAFETY: `data` is a valid pointer from `CMSG_DATA`;
                // `i` is bounded by `nfds` derived from `cmsg_len`.
                let raw_fd = unsafe { *data.add(i) };
                if got_fd.is_none() {
                    // SAFETY: `raw_fd` comes from kernel `SCM_RIGHTS`;
                    // `MSG_CMSG_CLOEXEC` was set on `recvmsg`.
                    got_fd = Some(unsafe { LocalFd::from_raw(raw_fd) });
                } else {
                    // SAFETY: `raw_fd` is a valid fd from the kernel;
                    // close of a valid fd is safe.
                    unsafe { libc::close(raw_fd) };
                }
            }
        } else if level == libc::SOL_SOCKET && ctype == libc::SCM_CREDENTIALS {
            // SAFETY: `cmsg` is a valid `cmsghdr` pointer.
            let payload = (unsafe { (*cmsg).cmsg_len } as usize)
                .saturating_sub(core::mem::size_of::<libc::cmsghdr>());
            ensure!(
                payload >= core::mem::size_of::<libc::ucred>(),
                crate::RecvFdError::Never
            );
            // SAFETY: `cmsg` is a valid `cmsghdr` with `SCM_CREDENTIALS`;
            // the kernel always provides a full `ucred`.
            let cred = unsafe { &*libc::CMSG_DATA(cmsg).cast::<libc::ucred>() };
            got_pid = Some(cred.pid);
        }
        // SAFETY: `msg` and `cmsg` are valid pointers; `CMSG_NXTHDR`
        // returns null at end or on malformed data (safe).
        cmsg = unsafe { libc::CMSG_NXTHDR(&msg, cmsg) };
    }

    let fd = got_fd.ok_or(crate::RecvFdError::NoFd)?;
    if let Some(pid) = got_pid
        && pid != expected_pid
    {
        bail!(crate::RecvFdError::PidMismatch(pid, expected_pid));
    }

    let tag_slice = tag.get(..n).ok_or(crate::RecvFdError::TagTooLong)?;
    let tag_cstr =
        CStr::from_bytes_with_nul(tag_slice).change_context(crate::RecvFdError::TagNotNul)?;
    Ok((fd, tag_cstr))
}
