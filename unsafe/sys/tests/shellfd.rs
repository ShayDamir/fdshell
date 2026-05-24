use sys::fd::{close, dup2};
use sys::net::socketpair;
use sys::pipe::pipe2;
use sys::rw::{read, write};
use sys::shellfd::{recv_fd, send_fd, SHELLFD, TAG_MAX};

#[repr(C)]
struct CmsgBuf {
    hdr: libc::cmsghdr,
    fd: libc::c_int,
}

fn send_raw_msg(fd: i32, tag_bytes: &[u8], send_fd: i32) -> Result<(), i32> {
    let iov = libc::iovec {
        iov_base: tag_bytes.as_ptr() as *mut core::ffi::c_void,
        iov_len: tag_bytes.len(),
    };
    let mut cmsg = CmsgBuf {
        hdr: libc::cmsghdr {
            // SAFETY: `CMSG_LEN(4)` returns the size of a cmsghdr + one i32,
            // always valid on x86_64 Linux.
            cmsg_len: unsafe { libc::CMSG_LEN(4) as usize },
            cmsg_level: libc::SOL_SOCKET,
            cmsg_type: libc::SCM_RIGHTS,
        },
        fd: send_fd,
    };
    let msg = libc::msghdr {
        msg_name: core::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: &iov as *const libc::iovec as *mut libc::iovec,
        msg_iovlen: 1,
        msg_control: &mut cmsg as *mut CmsgBuf as *mut core::ffi::c_void,
        msg_controllen: core::mem::size_of::<CmsgBuf>(),
        msg_flags: 0,
    };
    // SAFETY: `iov`, `cmsg`, `msg` are valid stack-local values. `fd` is a
    // connected Unix socket. `send_fd` is a valid open fd.
    if unsafe { libc::sendmsg(fd, &msg, 0) } == -1 {
        // SAFETY: `__errno_location()` returns a valid pointer to thread-local errno.
        return Err(unsafe { *libc::__errno_location() });
    }
    Ok(())
}

#[test]
fn test_send_recv_fd() -> Result<(), i32> {
    let mut pair = [0; 2];
    socketpair(&mut pair)?;

    if pair[0] != SHELLFD {
        dup2(pair[0], SHELLFD)?;
        let _ = close(pair[0]);
    }
    let receiver = pair[1];

    let mut test_pair = [0; 2];
    socketpair(&mut test_pair)?;

    send_fd(test_pair[0], c"test")?;
    let _ = close(test_pair[0]);
    write(test_pair[1], b"42")?;
    let _ = close(test_pair[1]);

    let mut tag = [0u8; TAG_MAX];
    let (test_fd, _tag) = recv_fd(receiver, &mut tag)?;

    let mut buf = [0u8; 8];
    assert_eq!(read(test_fd, &mut buf)?, 2);
    assert_eq!(&buf[..2], b"42");
    assert_eq!(read(test_fd, &mut buf)?, 0);

    let _ = close(test_fd);
    let _ = close(receiver);
    Ok(())
}

#[test]
fn test_recv_fd_truncated() -> Result<(), i32> {
    // 8192 bytes fills tag buffer + spills into extra → n > TAG_MAX
    let mut pair = [0; 2];
    socketpair(&mut pair)?;
    let (dummy_rd, dummy_wr) = pipe2(0)?;

    let tag = [b'x'; 8192];
    send_raw_msg(pair[0], &tag, dummy_wr)?;
    let _ = close(dummy_wr);

    let mut buf = [0u8; TAG_MAX];
    assert_eq!(recv_fd(pair[1], &mut buf), Err(libc::EINVAL));

    let _ = close(dummy_rd);
    let _ = close(pair[0]);
    let _ = close(pair[1]);
    Ok(())
}

#[test]
fn test_recv_fd_exact_size_no_null() -> Result<(), i32> {
    // Exactly TAG_MAX bytes, no null → CStr::from_bytes_with_nul fails
    let mut pair = [0; 2];
    socketpair(&mut pair)?;
    let (dummy_rd, dummy_wr) = pipe2(0)?;

    let tag = [b'x'; TAG_MAX];
    send_raw_msg(pair[0], &tag, dummy_wr)?;
    let _ = close(dummy_wr);

    let mut buf = [0u8; TAG_MAX];
    assert_eq!(recv_fd(pair[1], &mut buf), Err(libc::EINVAL));

    let _ = close(dummy_rd);
    let _ = close(pair[0]);
    let _ = close(pair[1]);
    Ok(())
}

#[test]
fn test_recv_fd_short_no_null() -> Result<(), i32> {
    let mut pair = [0; 2];
    socketpair(&mut pair)?;
    let (dummy_rd, dummy_wr) = pipe2(0)?;

    send_raw_msg(pair[0], b"abc", dummy_wr)?;
    let _ = close(dummy_wr);

    let mut buf = [0u8; TAG_MAX];
    assert_eq!(recv_fd(pair[1], &mut buf), Err(libc::EINVAL));

    let _ = close(dummy_rd);
    let _ = close(pair[0]);
    let _ = close(pair[1]);
    Ok(())
}

#[test]
fn test_recv_fd_interior_null() -> Result<(), i32> {
    let mut pair = [0; 2];
    socketpair(&mut pair)?;
    let (dummy_rd, dummy_wr) = pipe2(0)?;

    send_raw_msg(pair[0], b"abc\0fde\0", dummy_wr)?;
    let _ = close(dummy_wr);

    let mut buf = [0u8; TAG_MAX];
    assert_eq!(recv_fd(pair[1], &mut buf), Err(libc::EINVAL));

    let _ = close(dummy_rd);
    let _ = close(pair[0]);
    let _ = close(pair[1]);
    Ok(())
}

#[test]
fn test_recv_fd_null_at_end_of_buffer() -> Result<(), i32> {
    let mut pair = [0; 2];
    socketpair(&mut pair)?;
    let (dummy_rd, dummy_wr) = pipe2(0)?;

    let mut tag = vec![b'x'; TAG_MAX - 1];
    tag.push(0);
    tag.extend_from_slice(b"rd\0");
    send_raw_msg(pair[0], &tag, dummy_wr)?;
    let _ = close(dummy_wr);

    let mut buf = [0u8; TAG_MAX];
    assert_eq!(recv_fd(pair[1], &mut buf), Err(libc::EINVAL));

    let _ = close(dummy_rd);
    let _ = close(pair[0]);
    let _ = close(pair[1]);
    Ok(())
}
