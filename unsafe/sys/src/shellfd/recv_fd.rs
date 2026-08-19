use error_stack::{Report, ResultExt, bail, ensure};

use crate::LocalFd;
use crate::iovec::IoVecMut;
use core::ffi::CStr;

#[repr(align(8))]
struct CtrlBuf([u8; 64]);

pub fn recv_fd<'a>(
    sock: &LocalFd,
    tag: &'a mut [u8],
    expected_pid: crate::Pid,
) -> Result<(LocalFd, &'a CStr), Report<crate::RecvFdError>> {
    let mut extra = [0u8; 1];
    // SCM_RIGHTS (1 fd: 24 B) + SCM_CREDENTIALS (1 ucred: 32 B) = 56 B
    let mut ctrl_buf = CtrlBuf([0; 64]);
    let mut iovs = [IoVecMut::new(tag), IoVecMut::new(&mut extra)];
    let mut msg = libc::msghdr {
        msg_name: core::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: iovs.as_mut_ptr().cast(),
        msg_iovlen: 2,
        msg_control: ctrl_buf.0.as_mut_ptr().cast(),
        msg_controllen: ctrl_buf.0.len(),
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
        if (level, ctype) == (libc::SOL_SOCKET, libc::SCM_RIGHTS) {
            // SAFETY: `cmsg` is a valid `cmsghdr`; `CMSG_DATA` is valid for
            // `cmsg_len` bytes and 8-byte aligned, satisfying i32's alignment.
            let nfds = ((unsafe { (*cmsg).cmsg_len } as usize)
                .saturating_sub(core::mem::size_of::<libc::cmsghdr>()))
                / core::mem::size_of::<i32>();
            // SAFETY: pointer is valid for `nfds` i32s within the `CMSG_DATA` region.
            let fds =
                unsafe { core::slice::from_raw_parts(libc::CMSG_DATA(cmsg).cast::<i32>(), nfds) };
            // Take the first fd, close any further fds sent in the same message.
            if let Some((first, rest)) = fds.split_first() {
                // SAFETY: `first` comes from kernel `SCM_RIGHTS`;
                // `MSG_CMSG_CLOEXEC` was set on `recvmsg`.
                got_fd = Some(unsafe { LocalFd::from_raw(*first) });
                for &raw_fd in rest {
                    // SAFETY: `raw_fd` is a valid fd from the kernel; close is safe.
                    crate::cvt(unsafe { libc::close(raw_fd) as isize })
                        .change_context(crate::RecvFdError::Never)?;
                }
            }
        } else if (level, ctype) == (libc::SOL_SOCKET, libc::SCM_CREDENTIALS) {
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
        && pid != expected_pid.as_raw()
    {
        bail!(crate::RecvFdError::PidMismatch(pid, expected_pid.as_raw()));
    }

    let tag_slice = tag.get(..n).ok_or(crate::RecvFdError::TagTooLong)?;
    let tag_cstr =
        CStr::from_bytes_with_nul(tag_slice).change_context(crate::RecvFdError::TagNotNul)?;
    Ok((fd, tag_cstr))
}
