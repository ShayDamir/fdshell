//! `sendmsg` — send a byte payload plus fd vars in one SCM_RIGHTS message.

use error_stack::{Report, ensure};

use super::cmsg::{MAX_FDS, rights_space};
use crate::{LocalFd, SyscallError, cvt};

/// `repr(C)` pins the field order (repr(Rust) may reorder `fds` before
/// `hdr` to kill padding); `align(8)` matches the cmsg data alignment.
#[repr(C, align(8))]
struct SendCtrl {
    hdr: libc::cmsghdr,
    fds: [libc::c_int; MAX_FDS],
}

/// Send `payload` with `fds` (SCM_RIGHTS) on `sock`.
///
/// The payload is one iovec; all fds travel in a single `SCM_RIGHTS` control
/// message attached to the first fragment. On a stream socket a payload larger
/// than the socket buffer is split across kernel fragments: the fds arrive
/// with the first fragment, the rest of the payload follows as plain bytes.
/// The call blocks until the whole payload is accepted (like `read`): on a
/// non-blocking socket with a full buffer it yields and retries on `EAGAIN`.
///
/// An empty payload with fds is rejected: on AF_UNIX stream sockets the
/// kernel sends nothing and silently drops every fd (`unix_stream_sendmsg`
/// runs its send loop zero times, then `scm_destroy` closes them).
pub fn sendmsg(
    sock: &LocalFd,
    payload: &[u8],
    fds: &[LocalFd],
) -> Result<(), Report<SyscallError>> {
    ensure!(fds.len() <= MAX_FDS, SyscallError::E2BIG("sendmsg"));
    ensure!(
        !payload.is_empty() || fds.is_empty(),
        SyscallError::EINVAL("sendmsg")
    );

    let n = fds.len();
    let mut ctrl = SendCtrl {
        hdr: libc::cmsghdr {
            cmsg_level: libc::SOL_SOCKET,
            cmsg_type: libc::SCM_RIGHTS,
            cmsg_len: rights_space(n) as _,
        },
        fds: [0; MAX_FDS],
    };
    for (slot, fd) in ctrl.fds.iter_mut().take(n).zip(fds.iter()) {
        *slot = fd.as_raw();
    }

    let mut off = 0usize;
    let mut ctrl_sent = false;
    while !ctrl_sent || off < payload.len() {
        // The control message rides exactly the first fragment. One condition
        // drives both msghdr fields: split across the two, a mutated pointer
        // arm with length 0 is an accidental equivalent (the kernel ignores a
        // control pointer its length says is empty). `is_empty` (not `n > 0`):
        // the `>→>=` mutant differs only for zero fds, where the attached
        // empty `SCM_RIGHTS` is silently dropped by the kernel (probed) — an
        // unkillable equivalent.
        let attach_ctrl = !ctrl_sent && !fds.is_empty();
        let mut iov = libc::iovec {
            iov_base: payload
                .get(off..)
                .ok_or(SyscallError::Never)?
                .as_ptr()
                .cast_mut()
                .cast(),
            iov_len: payload.len() - off,
        };
        let msg = libc::msghdr {
            msg_name: core::ptr::null_mut(),
            msg_namelen: 0,
            msg_iov: &raw mut iov,
            msg_iovlen: 1,
            msg_control: if attach_ctrl {
                &raw mut ctrl
            } else {
                core::ptr::null_mut()
            }
            .cast(),
            msg_controllen: if attach_ctrl { rights_space(n) } else { 0 },
            msg_flags: 0,
        };
        // SAFETY: `sock` is a valid open socket. `iov` names a valid slice of
        // `payload`. `msg` is a valid stack allocation. When the control buffer
        // is attached, `ctrl.hdr` is a valid `SCM_RIGHTS` cmsg whose `cmsg_len`
        // names exactly the `n` fds filled above. The kernel only reads within
        // those bounds.
        let sent = match cvt(unsafe { libc::sendmsg(sock.as_raw(), &raw const msg, 0) }) {
            Ok(sent) => sent,
            // The buffer is full: yield and retry so the call blocks until the
            // whole payload is accepted, like `read`.
            Err(SyscallError::EAGAIN(_)) => {
                // SAFETY: `sched_yield` takes no arguments and cannot fault.
                unsafe { libc::sched_yield() };
                continue;
            }
            Err(e) => return Err(Report::new(e)),
        };
        // A unix stream either sends bytes or fails; `sent == 0` with bytes
        // left is an impossible state (the empty-payload no-op ends the loop
        // via `off == payload.len()`).
        ensure!(sent > 0 || off == payload.len(), SyscallError::Never);
        off += sent as usize;
        ctrl_sent = true;
    }
    Ok(())
}
