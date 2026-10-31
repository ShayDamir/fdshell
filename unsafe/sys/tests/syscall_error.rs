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

/// Every table entry renders its standard description: bare when the syscall
/// is the placeholder (the `.into()` path — the shape bash prints after the
/// word), with the syscall appended when it is named. Both arms per entry, so
/// neither is an unkillable equivalent mutant (task #374).
#[test]
fn display_renders_standard_descriptions() {
    let table = [
        (libc::E2BIG, "Argument list too long"),
        (libc::EACCES, "Permission denied"),
        (libc::EAGAIN, "Resource temporarily unavailable"),
        (libc::EBADF, "Bad file descriptor"),
        (libc::EEXIST, "File exists"),
        (libc::EINVAL, "Invalid argument"),
        (libc::EIO, "Input/output error"),
        (libc::EISDIR, "Is a directory"),
        (libc::ELOOP, "Too many levels of symbolic links"),
        (libc::EMFILE, "Too many open files"),
        (libc::ENAMETOOLONG, "File name too long"),
        (libc::ENOENT, "No such file or directory"),
        (libc::ENOSYS, "Function not implemented"),
        (libc::ENOTDIR, "Not a directory"),
        (libc::EPERM, "Operation not permitted"),
    ];
    for (errno, desc) in table {
        let bare: SyscallError = errno.into();
        assert_eq!(format!("{bare}"), desc, "errno {errno} via the placeholder");
        let named = SyscallError::Other {
            errno,
            syscall: "openat2",
        };
        assert_eq!(
            format!("{named}"),
            format!("{desc} (openat2)"),
            "errno {errno} with a named syscall"
        );
    }
}

#[test]
fn display_named_variants() {
    assert_eq!(
        format!("{}", SyscallError::EINVAL("waitid")),
        "Invalid argument (waitid)"
    );
    assert_eq!(
        format!("{}", SyscallError::EBADF("fchdir")),
        "Bad file descriptor (fchdir)"
    );
}

#[test]
fn display_other_variant() {
    let o = SyscallError::Other {
        errno: 22,
        syscall: "sendmsg",
    };
    assert_eq!(format!("{}", o), "Invalid argument (sendmsg)");
}

#[test]
fn display_never() {
    // `Never` is an invariant violation, not an errno (STYLE 4.10): it keeps
    // its own name even though `errno()` answers 0 for it.
    assert_eq!(format!("{}", SyscallError::Never), "Never");
}

#[test]
fn from_errno_known() {
    let e: SyscallError = libc::EEXIST.into();
    assert_eq!(e.errno(), libc::EEXIST);
    assert_eq!(e.syscall(), "unknown");
    assert_eq!(format!("{}", e), "File exists");
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
