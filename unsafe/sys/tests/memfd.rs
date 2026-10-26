#![allow(clippy::unwrap_used)]

use sys::memfd::{
    F_SEAL_WRITE, MFD_ALLOW_SEALING, MFD_CLOEXEC, memfd_create, memfd_create_with_name_and_flags,
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
    fd.set_seals(F_SEAL_WRITE as u32).unwrap();
}

/// Without `MFD_ALLOW_SEALING` the memfd starts with `F_SEAL_SEAL` applied,
/// so adding any seal fails with EPERM (the "already sealed" check fires
/// before any feature check).
#[test]
fn seal_fails_without_allow_sealing() {
    let fd = memfd_create_with_name_and_flags(None, MFD_CLOEXEC).unwrap();
    let err = fd.set_seals(F_SEAL_WRITE as u32).unwrap_err();
    assert_eq!(err.errno(), libc::EPERM);
}

/// The read-only re-open is a **fresh open file description**: its offset starts
/// at 0 whatever the memfd fd's own offset is (the write left it at EOF), the
/// bytes read back, and the fd is a `LocalFd` — `CLOEXEC` set, the invariant a
/// capture-based test cannot otherwise see (LESSONS: "A received `SCM_RIGHTS` fd
/// is always `CLOEXEC`…"). Task #325: the offset-0 start is what lets the
/// here-doc arms drop their `lseek(0, SEEK_SET)`.
#[test]
fn reopen_read_only_hands_back_a_fresh_fd_at_offset_zero() {
    let fd = memfd_create().unwrap();
    fd.write_all(b"hi\n").unwrap();
    let ro = fd.reopen_read_only().unwrap();
    let mut buf = [0u8; 8];
    let n = ro.read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"hi\n");
    assert_eq!(ro.read(&mut buf).unwrap(), 0, "the body ends at EOF");
    ro.verify().unwrap();
}

/// Writes to the read-only re-open are refused `EBADF` — bash's diagnostic for
/// a write to its heredoc/here-string source — and the mode travels with the
/// open file description, so a `dup` of it refuses writes too. That is the
/// property the `>&N` rows of tasks #276/#312/#325 lean on: `exec 4<&3` shares
/// fd 3's mode.
#[test]
fn reopen_read_only_refuses_writes() {
    let fd = memfd_create().unwrap();
    fd.write_all(b"hi\n").unwrap();
    let ro = fd.reopen_read_only().unwrap();
    let err = ro.write(b"x").unwrap_err();
    assert_eq!(err.errno(), libc::EBADF);
    let dup = ro.try_clone().unwrap();
    assert_eq!(
        dup.write(b"x").unwrap_err().errno(),
        libc::EBADF,
        "the mode is a property of the open file description, not the fd"
    );
    // The body survives the refused write: no data loss in the shell's table.
    let mut buf = [0u8; 8];
    let n = dup.read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"hi\n");
}
