//! Shared cmsg size math and the kernel control-buffer walk for the
//! `sendmsg`/`recvmsg` wrappers.

use alloc::vec::Vec;

use crate::LocalFd;

/// Maximum number of fds a single `SCM_RIGHTS` cmsg can carry.
pub const MAX_FDS: usize = 64;

/// `CMSG_SPACE(n * sizeof(c_int))`: bytes a `SCM_RIGHTS` cmsg with `n` fds
/// occupies in the control buffer (the header is already 8-byte aligned).
pub(crate) const fn rights_space(n: usize) -> usize {
    core::mem::size_of::<libc::cmsghdr>() + n * core::mem::size_of::<libc::c_int>()
}

/// `CMSG_SPACE(sizeof(struct ucred))`: bytes a `SCM_CREDENTIALS` cmsg occupies.
pub(crate) const fn cred_space() -> usize {
    (core::mem::size_of::<libc::cmsghdr>() + core::mem::size_of::<libc::ucred>() + 7) & !7
}

/// Largest control buffer: `MAX_FDS` rights plus one `SCM_CREDENTIALS`.
pub(crate) const MAX_CTRL: usize = rights_space(MAX_FDS) + cred_space();

/// Walk the kernel-written cmsgs of a received `msghdr`, taking ownership of
/// the `SCM_RIGHTS` fds in `fds` and the `SCM_CREDENTIALS` in `cred`.
pub(crate) fn walk(
    msg: *mut libc::msghdr,
    fds: &mut Vec<LocalFd>,
    cred: &mut Option<(i32, u32, u32)>,
) {
    // SAFETY: `CMSG_FIRSTHDR` returns a pointer inside `msg_control` or null
    // when no control data fits.
    let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(msg) };
    while !cmsg.is_null() {
        // SAFETY: `cmsg` is non-null and points at a `cmsghdr` written by the
        // kernel within the control buffer.
        let (level, ctype, len) = unsafe {
            (
                (*cmsg).cmsg_level,
                (*cmsg).cmsg_type,
                (*cmsg).cmsg_len as usize,
            )
        };
        if (level, ctype) == (libc::SOL_SOCKET, libc::SCM_RIGHTS) {
            push_rights(cmsg, len, fds);
        } else if (level, ctype) == (libc::SOL_SOCKET, libc::SCM_CREDENTIALS) {
            *cred = Some(read_ucred(cmsg));
        }
        // SAFETY: `CMSG_NXTHDR` walks within `msg_controllen` or returns null
        // at the end (or on malformed data, which cannot occur kernel-written).
        cmsg = unsafe { libc::CMSG_NXTHDR(msg, cmsg) };
    }
}

fn push_rights(cmsg: *const libc::cmsghdr, len: usize, fds: &mut Vec<LocalFd>) {
    let count =
        len.saturating_sub(core::mem::size_of::<libc::cmsghdr>()) / core::mem::size_of::<i32>();
    // SAFETY: `CMSG_DATA` is 8-byte aligned and valid for `len - cmsg header`
    // bytes, covering `count` i32s; the kernel wrote them.
    let raw = unsafe { core::slice::from_raw_parts(libc::CMSG_DATA(cmsg).cast::<i32>(), count) };
    for &r in raw {
        // SAFETY: `r` is a kernel SCM_RIGHTS dup; `MSG_CMSG_CLOEXEC` was set,
        // so the CLOEXEC invariant holds.
        fds.push(unsafe { LocalFd::from_raw(r) });
    }
}

fn read_ucred(cmsg: *const libc::cmsghdr) -> (i32, u32, u32) {
    // SAFETY: `cmsg` carries SCM_CREDENTIALS; the kernel always fills a full
    // `ucred` (CMSG_DATA is 8-byte aligned, ucred needs 4).
    let ucred = unsafe { &*libc::CMSG_DATA(cmsg).cast::<libc::ucred>() };
    (ucred.pid, ucred.uid, ucred.gid)
}
