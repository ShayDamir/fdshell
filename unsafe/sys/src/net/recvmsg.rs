//! `recvmsg` — receive a byte payload plus fd vars from a socket.

use alloc::vec::Vec;
use error_stack::{Report, ensure};

use super::cmsg::{MAX_CTRL, MAX_FDS, cred_space, rights_space, walk};
use crate::{LocalFd, SyscallError, cvt};

/// A received message: `payload_len` valid bytes in the caller's buffer, the
/// received fds in cmsg order (each owned, closed on drop), and the sender
/// credentials when `cred` was requested.
pub struct Msg {
    pub payload_len: usize,
    pub fds: Vec<LocalFd>,
    /// `(pid, uid, gid)` of the sender.
    pub cred: Option<(i32, u32, u32)>,
    /// Peer closed the connection: no data and no fds.
    pub eof: bool,
}

impl core::fmt::Debug for Msg {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Msg")
            .field("payload_len", &self.payload_len)
            .field("fds", &self.fds.len())
            .field("cred", &self.cred)
            .field("eof", &self.eof)
            .finish()
    }
}

#[repr(align(8))]
struct RecvCtrl([u8; MAX_CTRL]);

/// Receive up to `buf.len()` payload bytes and up to `max_fds` fds (SCM_RIGHTS)
/// from `sock`. With `cred`, sets nothing — the caller must have enabled
/// `SO_PASSCRED` — and also harvests the `SCM_CREDENTIALS` cmsg.
///
/// Fds are received with `MSG_CMSG_CLOEXEC`. More fds from the peer than the
/// buffer holds: the kernel drops the excess and sets `MSG_CTRUNC`; the
/// wrapper returns `E2BIG`, closing the fds it did receive.
pub fn recvmsg(
    sock: &LocalFd,
    buf: &mut [u8],
    max_fds: usize,
    cred: bool,
) -> Result<Msg, Report<SyscallError>> {
    ensure!(max_fds <= MAX_FDS, SyscallError::E2BIG("recvmsg"));
    let mut ctrl = RecvCtrl([0u8; MAX_CTRL]);
    let controllen = rights_space(max_fds) + if cred { cred_space() } else { 0 };
    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr().cast(),
        iov_len: buf.len(),
    };
    let mut msg = libc::msghdr {
        msg_name: core::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: &raw mut iov,
        msg_iovlen: 1,
        msg_control: ctrl.0.as_mut_ptr().cast(),
        msg_controllen: controllen,
        msg_flags: 0,
    };
    // SAFETY: `sock` is a valid open socket. `msg`, `iov`, and `ctrl` are valid
    // stack allocations; `recvmsg` only writes within their bounds.
    let n = cvt(unsafe { libc::recvmsg(sock.as_raw(), &raw mut msg, libc::MSG_CMSG_CLOEXEC) })?
        as usize;
    // The kernel dups in-buffer fds into our table at delivery, so walk the
    // cmsgs first and own them in `fds`; the `MSG_CTRUNC` check below then
    // closes them on the error path by dropping the vec.
    let mut fds: Vec<LocalFd> = Vec::new();
    let mut cred_out: Option<(i32, u32, u32)> = None;
    walk(&raw mut msg, &mut fds, &mut cred_out);
    // The kernel drops fds the control buffer cannot hold and sets MSG_CTRUNC.
    ensure!(
        msg.msg_flags & libc::MSG_CTRUNC == 0,
        SyscallError::E2BIG("recvmsg")
    );

    Ok(Msg {
        payload_len: n,
        fds,
        cred: cred_out,
        // A stream socket never delivers fds with a zero-length message, so a
        // bare 0 return is the peer closing the write side.
        eof: n == 0,
    })
}
