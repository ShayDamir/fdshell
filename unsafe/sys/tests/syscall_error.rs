use sys::SyscallError;

#[test]
fn errno_roundtrips() {
    assert_eq!(SyscallError::E2BIG("x").errno(), libc::E2BIG);
    assert_eq!(SyscallError::EAGAIN("x").errno(), libc::EAGAIN);
    assert_eq!(SyscallError::EBADF("x").errno(), libc::EBADF);
    assert_eq!(SyscallError::EEXIST("x").errno(), libc::EEXIST);
    assert_eq!(SyscallError::EINVAL("x").errno(), libc::EINVAL);
    assert_eq!(SyscallError::EIO("x").errno(), libc::EIO);
    assert_eq!(SyscallError::EMFILE("x").errno(), libc::EMFILE);
    assert_eq!(SyscallError::ENOENT("x").errno(), libc::ENOENT);
    assert_eq!(SyscallError::ENOSYS("x").errno(), libc::ENOSYS);
    assert_eq!(SyscallError::EPERM("x").errno(), libc::EPERM);
    let o = SyscallError::Other {
        errno: 99,
        syscall: "test",
    };
    assert_eq!(o.errno(), 99);
}

#[test]
fn syscall_name_accessor() {
    assert_eq!(
        SyscallError::EINVAL("fcntl(F_GETFD)").syscall(),
        "fcntl(F_GETFD)"
    );
    let o = SyscallError::Other {
        errno: 42,
        syscall: "sendmsg",
    };
    assert_eq!(o.syscall(), "sendmsg");
}

#[test]
fn display_named_variants() {
    assert_eq!(
        format!("{}", SyscallError::EINVAL("waitid")),
        "EINVAL (waitid)"
    );
    assert_eq!(
        format!("{}", SyscallError::EBADF("fchdir")),
        "EBADF (fchdir)"
    );
}

#[test]
fn display_other_variant() {
    let o = SyscallError::Other {
        errno: 22,
        syscall: "sendmsg",
    };
    assert_eq!(format!("{}", o), "sendmsg (errno 22)");
}

#[test]
fn from_errno_known() {
    let e: SyscallError = libc::EEXIST.into();
    assert_eq!(e.errno(), libc::EEXIST);
    assert_eq!(e.syscall(), "unknown");
    assert_eq!(format!("{}", e), "EEXIST (unknown)");
}

#[test]
fn from_errno_unknown() {
    let e: SyscallError = 0xDEAD.into();
    assert_eq!(e.errno(), 0xDEAD);
    assert_eq!(e.syscall(), "unknown");
    assert_eq!(format!("{}", e), "unknown (errno 57005)");
}

#[test]
fn from_errno_einval() {
    let e: SyscallError = libc::EINVAL.into();
    assert!(matches!(e, SyscallError::EINVAL(_)));
    assert_eq!(e.syscall(), "unknown");
}
