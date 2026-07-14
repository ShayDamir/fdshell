#![allow(clippy::unwrap_used, clippy::expect_used)]

use sys::SyscallError;
use sys::net::{set_passcred, socketpair};
use sys::pipe::pipe2;
use sys::rw::{read, write};
use sys::shellfd::{RecvFdError, TAG_MAX, recv_fd, send_fd, set_capture_active};

#[repr(C)]
struct CmsgBuf {
    hdr: libc::cmsghdr,
    fd: libc::c_int,
}

fn send_raw_msg(fd: i32, tag_bytes: &[u8], send_fd: i32) -> Result<(), SyscallError> {
    let mut iov = libc::iovec {
        iov_base: tag_bytes.as_ptr().cast_mut().cast(),
        iov_len: tag_bytes.len(),
    };
    let mut cmsg = CmsgBuf {
        hdr: libc::cmsghdr {
            // SAFETY: `CMSG_LEN(4)` returns the size of a cmsghdr + one i32,
            // always valid on x86_64 Linux.
            cmsg_len: unsafe { libc::CMSG_LEN(core::mem::size_of::<i32>() as u32) as usize },
            cmsg_level: libc::SOL_SOCKET,
            cmsg_type: libc::SCM_RIGHTS,
        },
        fd: send_fd,
    };
    let msg = libc::msghdr {
        msg_name: core::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: &raw mut iov,
        msg_iovlen: 1,
        msg_control: (&raw mut cmsg).cast(),
        msg_controllen: core::mem::size_of_val(&cmsg),
        msg_flags: 0,
    };
    // SAFETY: `iov`, `cmsg`, `msg` are valid stack-local values; `fd` is a
    // connected Unix socket; `send_fd` is a valid open fd.
    if unsafe { libc::sendmsg(fd, &msg, 0) } == -1 {
        // SAFETY: `__errno_location()` returns a valid pointer to thread-local errno.
        return Err(unsafe {
            SyscallError::Other {
                errno: *libc::__errno_location(),
                syscall: "sendmsg",
            }
        });
    }
    Ok(())
}

fn fork_test(f: fn() -> Result<(), SyscallError>) -> Result<(), SyscallError> {
    // SAFETY: child inherits a copy of the fd table; parent waits for it.
    let pid = unsafe { libc::fork() };
    if pid == -1 {
        // SAFETY: `__errno_location()` returns a valid pointer to thread-local errno.
        return Err(unsafe {
            SyscallError::Other {
                errno: *libc::__errno_location(),
                syscall: "fork",
            }
        });
    }
    if pid == 0 {
        let code = match &f() {
            Ok(()) => 0,
            Err(e) => e.errno(),
        };
        sys::exit(code);
    }
    let mut status = 0i32;
    // SAFETY: waitpid for our direct child blocks until it exits.
    unsafe { libc::waitpid(pid, &mut status, 0) };
    if libc::WIFEXITED(status) {
        let code = libc::WEXITSTATUS(status);
        if code == 0 {
            Ok(())
        } else {
            Err(SyscallError::Other {
                errno: code as i32,
                syscall: "fork_test",
            })
        }
    } else if libc::WIFSIGNALED(status) {
        panic!("test child killed by signal {}", libc::WTERMSIG(status));
    } else {
        panic!("test child terminated unexpectedly");
    }
}

#[test]
fn test_send_recv_fd() -> Result<(), SyscallError> {
    fork_test(|| {
        let (shell_a, shell_b) = socketpair()?;
        shell_a.verify()?;
        shell_b.verify()?;
        let receiver = shell_b;
        set_capture_active(true);

        shell_a.export()?;
        let shell_sock = shell_a.try_clone()?;
        shell_a.try_close()?;
        // shell_b stays open — it's the receive end of shell_sock

        let (test_a, test_b) = socketpair()?;
        test_a.verify()?;
        test_b.verify()?;
        send_fd(&shell_sock, &test_a, c"test")?;
        test_a.try_close()?;
        write(&test_b, b"42")?;
        test_b.try_close()?;

        let mut tag = [0u8; TAG_MAX];
        let (test_fd, _tag) = recv_fd(&receiver, &mut tag, std::process::id() as i32)
            .expect("recv_fd should succeed");
        test_fd.verify()?;

        let mut buf = [0u8; 8];
        assert_eq!(read(&test_fd, &mut buf)?, 2);
        assert_eq!(&buf[..2], b"42");
        assert_eq!(read(&test_fd, &mut buf)?, 0);

        test_fd.try_close()?;
        receiver.try_close()?;
        Ok(())
    })
}

#[test]
fn test_recv_fd_truncated() -> Result<(), SyscallError> {
    let (a, b) = socketpair()?;
    a.verify()?;
    b.verify()?;
    let (dummy_rd, dummy_wr) = pipe2(libc::O_CLOEXEC)?;
    dummy_rd.verify()?;
    dummy_wr.verify()?;

    let tag = [b'x'; 2 * TAG_MAX];
    send_raw_msg(a.as_raw(), &tag, dummy_wr.as_raw())?;
    dummy_wr.try_close()?;

    let mut buf = [0u8; TAG_MAX];
    match recv_fd(&b, &mut buf, 0) {
        Err(e) if matches!(*e.current_context(), RecvFdError::TagTooLong) => {}
        _other => panic!("expected TagTooLong"),
    }

    dummy_rd.try_close()?;
    a.try_close()?;
    b.try_close()?;
    Ok(())
}

#[test]
fn test_recv_fd_exact_size_no_null() -> Result<(), SyscallError> {
    let (a, b) = socketpair()?;
    a.verify()?;
    b.verify()?;
    let (dummy_rd, dummy_wr) = pipe2(libc::O_CLOEXEC)?;
    dummy_rd.verify()?;
    dummy_wr.verify()?;

    let tag = [b'x'; TAG_MAX];
    send_raw_msg(a.as_raw(), &tag, dummy_wr.as_raw())?;
    dummy_wr.try_close()?;

    let mut buf = [0u8; TAG_MAX];
    match recv_fd(&b, &mut buf, 0) {
        Err(e) if matches!(*e.current_context(), RecvFdError::TagNotNul) => {}
        _other => panic!("expected TagNotNul"),
    }

    dummy_rd.try_close()?;
    a.try_close()?;
    b.try_close()?;
    Ok(())
}

#[test]
fn test_recv_fd_short_no_null() -> Result<(), SyscallError> {
    let (a, b) = socketpair()?;
    a.verify()?;
    b.verify()?;
    let (dummy_rd, dummy_wr) = pipe2(libc::O_CLOEXEC)?;
    dummy_rd.verify()?;
    dummy_wr.verify()?;

    send_raw_msg(a.as_raw(), b"abc", dummy_wr.as_raw())?;
    dummy_wr.try_close()?;

    let mut buf = [0u8; TAG_MAX];
    match recv_fd(&b, &mut buf, 0) {
        Err(e) if matches!(*e.current_context(), RecvFdError::TagNotNul) => {}
        _other => panic!("expected TagNotNul"),
    }

    dummy_rd.try_close()?;
    a.try_close()?;
    b.try_close()?;
    Ok(())
}

#[test]
fn test_recv_fd_interior_null() -> Result<(), SyscallError> {
    let (a, b) = socketpair()?;
    a.verify()?;
    b.verify()?;
    let (dummy_rd, dummy_wr) = pipe2(libc::O_CLOEXEC)?;
    dummy_rd.verify()?;
    dummy_wr.verify()?;

    send_raw_msg(a.as_raw(), b"abc\0fde\0", dummy_wr.as_raw())?;
    dummy_wr.try_close()?;

    let mut buf = [0u8; TAG_MAX];
    match recv_fd(&b, &mut buf, 0) {
        Err(e) if matches!(*e.current_context(), RecvFdError::TagNotNul) => {}
        _other => panic!("expected TagNotNul"),
    }

    dummy_rd.try_close()?;
    a.try_close()?;
    b.try_close()?;
    Ok(())
}

#[test]
fn test_recv_fd_truncated_creds() -> Result<(), SyscallError> {
    // The kernel rejects truncated SCM_CREDENTIALS at send time (EINVAL).
    // This test verifies that behavior and documents the missing cmsg_len
    // check as a defense-in-depth concern in recv_fd.
    let (a, b) = socketpair()?;
    a.verify()?;
    b.verify()?;
    set_passcred(&b)?;
    let (dummy_rd, mut dummy_wr) = pipe2(libc::O_CLOEXEC)?;
    dummy_rd.verify()?;
    dummy_wr.verify()?;

    // SAFETY: `CMSG_SPACE(0)` returns the minimum space for a control
    // message header, a valid constant on x86_64 Linux.
    let truncated = unsafe { libc::CMSG_SPACE(0) as usize };
    let mut ctrl = vec![0u8; truncated];
    // SAFETY: `ctrl` has `truncated` bytes, enough for `cmsghdr`.
    let cmsg = unsafe { &mut *ctrl.as_mut_ptr().cast::<libc::cmsghdr>() };
    // SAFETY: `CMSG_LEN(0)` is valid on x86_64 Linux; stored, not dereferenced.
    cmsg.cmsg_len = unsafe { libc::CMSG_LEN(0) as usize };
    cmsg.cmsg_level = libc::SOL_SOCKET;
    cmsg.cmsg_type = libc::SCM_CREDENTIALS;

    let mut iov = libc::iovec {
        iov_base: (&raw mut dummy_wr).cast(),
        iov_len: 1,
    };
    let msg = libc::msghdr {
        msg_name: core::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: &raw mut iov,
        msg_iovlen: 1,
        msg_control: ctrl.as_mut_ptr().cast(),
        msg_controllen: truncated,
        msg_flags: 0,
    };
    // SAFETY: `msg` and `ctrl` are valid stack allocations; `a` is a
    // connected Unix socket. Kernel rejects truncated creds — this
    // is expected.
    let ret = unsafe { libc::sendmsg(a.as_raw(), &msg, 0) };
    dummy_wr.try_close()?;

    assert_eq!(ret, -1);
    // SAFETY: `__errno_location()` returns a valid pointer to thread-local errno.
    let _e = unsafe { *libc::__errno_location() };

    dummy_rd.try_close()?;
    a.try_close()?;
    b.try_close()?;
    Ok(())
}

#[test]
fn test_recv_fd_null_at_end_of_buffer() -> Result<(), SyscallError> {
    let (a, b) = socketpair()?;
    a.verify()?;
    b.verify()?;
    let (dummy_rd, dummy_wr) = pipe2(libc::O_CLOEXEC)?;
    dummy_rd.verify()?;
    dummy_wr.verify()?;

    let mut tag = vec![b'x'; TAG_MAX - 1];
    tag.push(0);
    tag.extend_from_slice(b"rd\0");
    send_raw_msg(a.as_raw(), &tag, dummy_wr.as_raw())?;
    dummy_wr.try_close()?;

    let mut buf = [0u8; TAG_MAX];
    match recv_fd(&b, &mut buf, 0) {
        Err(e) if matches!(*e.current_context(), RecvFdError::TagTooLong) => {}
        _other => panic!("expected TagTooLong"),
    }

    dummy_rd.try_close()?;
    a.try_close()?;
    b.try_close()?;
    Ok(())
}

#[test]
fn test_recv_fd_pid_mismatch() -> Result<(), SyscallError> {
    let (a, b) = socketpair()?;
    a.verify()?;
    b.verify()?;
    set_passcred(&b)?;
    let (dummy_rd, dummy_wr) = pipe2(libc::O_CLOEXEC)?;
    dummy_rd.verify()?;
    dummy_wr.verify()?;

    let (fd_a, fd_b) = socketpair()?;
    fd_a.verify()?;
    fd_b.verify()?;
    send_raw_msg(a.as_raw(), b"test", fd_a.as_raw())?;
    fd_a.try_close()?;
    fd_b.try_close()?;

    let mut buf = [0u8; TAG_MAX];
    match recv_fd(&b, &mut buf, 0) {
        Err(e) if matches!(*e.current_context(), RecvFdError::PidMismatch(_, _)) => {}
        _other => panic!("expected PidMismatch"),
    }

    dummy_rd.try_close()?;
    a.try_close()?;
    b.try_close()?;
    Ok(())
}

#[test]
fn test_recv_fd_no_fd() -> Result<(), SyscallError> {
    let (a, b) = socketpair()?;
    a.verify()?;
    b.verify()?;
    set_passcred(&b)?;

    // Send a message without SCM_RIGHTS (msg_controllen == 0) to trigger NoFd.
    let mut iov = libc::iovec {
        iov_base: b"test".as_ptr().cast_mut().cast(),
        iov_len: 4,
    };
    let msg = libc::msghdr {
        msg_name: core::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: &raw mut iov,
        msg_iovlen: 1,
        msg_control: core::ptr::null_mut(),
        msg_controllen: 0,
        msg_flags: 0,
    };
    // SAFETY: `msg` is a valid stack-allocated msghdr with valid iovec; `sendmsg`
    // only reads from it. `a` is a valid open socket from socketpair().
    sys::cvt(unsafe { libc::sendmsg(a.as_raw(), &msg, 0) })?;

    let mut buf = [0u8; TAG_MAX];
    match recv_fd(&b, &mut buf, std::process::id() as i32) {
        Err(e) if matches!(*e.current_context(), RecvFdError::NoFd) => {}
        _other => panic!("expected NoFd"),
    }

    a.try_close()?;
    b.try_close()?;
    Ok(())
}
