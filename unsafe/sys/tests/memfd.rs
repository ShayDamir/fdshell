#![allow(clippy::unwrap_used)]

use sys::memfd::{
    F_SEAL_WRITE, MFD_ALLOW_SEALING, MFD_CLOEXEC, memfd_create, memfd_create_with_name_and_flags,
    memfd_set_seal,
};

/// The anonymous form must succeed. The kernel has no NULL special case — a
/// NULL name is copied via `strncpy_from_user` and faults with EFAULT
/// (errno 14); the wrapper must map `None` to the empty name.
#[test]
fn anonymous_memfd_is_created() {
    let _fd = memfd_create().unwrap();
}

#[test]
fn named_memfd_is_created() {
    let _fd = memfd_create_with_name_and_flags(Some(c"test"), MFD_CLOEXEC).unwrap();
}

#[test]
fn seal_succeeds_with_allow_sealing() {
    let fd = memfd_create_with_name_and_flags(None, MFD_CLOEXEC | MFD_ALLOW_SEALING).unwrap();
    memfd_set_seal(&fd, F_SEAL_WRITE as u32).unwrap();
}

/// Without `MFD_ALLOW_SEALING` the memfd starts with `F_SEAL_SEAL` applied,
/// so adding any seal fails with EPERM (the "already sealed" check fires
/// before any feature check).
#[test]
fn seal_fails_without_allow_sealing() {
    let fd = memfd_create_with_name_and_flags(None, MFD_CLOEXEC).unwrap();
    let err = memfd_set_seal(&fd, F_SEAL_WRITE as u32).unwrap_err();
    assert_eq!(err.errno(), libc::EPERM);
}
