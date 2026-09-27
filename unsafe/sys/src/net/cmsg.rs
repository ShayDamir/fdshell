//! Shared cmsg size math for the `sendmsg`/`recvmsg` wrappers.

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
