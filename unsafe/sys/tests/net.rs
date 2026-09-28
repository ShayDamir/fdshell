//! `sys::net::sendmsg` / `sys::net::recvmsg` integration tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use sys::SyscallError;
use sys::net::{recvmsg, sendmsg, socketpair};
use sys::poll::{POLLIN, PollFd, poll};

const BUF: usize = 64 * 1024;

fn buf() -> Vec<u8> {
    vec![0u8; BUF]
}

#[test]
fn test_sendmsg_recvmsg_payload_roundtrip() {
    let (a, b) = socketpair().unwrap();
    sendmsg(&a, b"hello", &[]).unwrap();
    let mut buf = buf();
    let msg = recvmsg(&b, &mut buf, 0, false).unwrap();
    assert!(!msg.eof);
    assert_eq!(&buf[..msg.payload_len], b"hello");
    assert!(msg.fds.is_empty());
    assert!(msg.cred.is_none());
    // The manual `Debug` impl prints the payload length (observes the body).
    assert!(format!("{msg:?}").contains("payload_len"));
}

#[test]
fn test_sendmsg_recvmsg_fd_roundtrip() {
    let (a, b) = socketpair().unwrap();
    let (c, peer) = socketpair().unwrap();
    sendmsg(&a, b"x", &[c]).unwrap();
    let mut buf = buf();
    let msg = recvmsg(&b, &mut buf, 1, false).unwrap();
    assert_eq!(&buf[..msg.payload_len], b"x");
    assert_eq!(msg.fds.len(), 1);
    // The received fd is a live dup of `c`: writing through it reaches the peer.
    msg.fds[0].write(b"ping").unwrap();
    let mut out = [0u8; 4];
    let n = peer.read(&mut out).unwrap();
    assert_eq!(&out[..n], b"ping");
}

#[test]
fn test_sendmsg_empty_payload_with_fds_fails() {
    let (a, _b) = socketpair().unwrap();
    let (c, _peer) = socketpair().unwrap();
    let err = sendmsg(&a, b"", &[c]).unwrap_err();
    assert_eq!(err.current_context(), &SyscallError::EINVAL("sendmsg"));
}

#[test]
fn test_sendmsg_empty_payload_no_fds_is_noop() {
    let (a, b) = socketpair().unwrap();
    sendmsg(&a, b"", &[]).unwrap();
    // A zero-length, cmsg-less send is invisible to the peer: not readable.
    let n = poll(&mut [PollFd::new(b.as_raw(), POLLIN)], 100).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn test_sendmsg_too_many_fds_fails() {
    let (a, _b) = socketpair().unwrap();
    let mut fds = Vec::new();
    for _ in 0..65 {
        let (c, _peer) = socketpair().unwrap();
        fds.push(c);
    }
    let err = sendmsg(&a, b"x", &fds).unwrap_err();
    assert_eq!(err.current_context(), &SyscallError::E2BIG("sendmsg"));
}

#[test]
fn test_recvmsg_eof() {
    let (a, b) = socketpair().unwrap();
    drop(a);
    let mut buf = buf();
    let msg = recvmsg(&b, &mut buf, 1, false).unwrap();
    assert!(msg.eof);
    assert_eq!(msg.payload_len, 0);
    assert!(msg.fds.is_empty());
}

#[test]
fn test_recvmsg_eagain_on_empty_nonblocking_socket() {
    let (a, b) = socketpair().unwrap();
    let _keep = a;
    let mut buf = buf();
    let err = recvmsg(&b, &mut buf, 1, false).unwrap_err();
    assert_eq!(err.current_context(), &SyscallError::EAGAIN("unknown"));
}

/// Number of fds open in this process (`/proc/self/fd` has one entry each).
fn open_fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

#[test]
fn test_recvmsg_truncates_excess_fds() {
    let (a, b) = socketpair().unwrap();
    let (c1, _p1) = socketpair().unwrap();
    let (c2, _p2) = socketpair().unwrap();
    let (c3, _p3) = socketpair().unwrap();
    sendmsg(&a, b"xy", &[c1, c2, c3]).unwrap();
    // The control buffer holds one fd; the kernel truncates the rest and has
    // already duped the in-buffer fd into our table.
    let before = open_fd_count();
    let mut buf = buf();
    let err = recvmsg(&b, &mut buf, 1, false).unwrap_err();
    assert_eq!(err.current_context(), &SyscallError::E2BIG("recvmsg"));
    // The error path must close the received fd: no net leak.
    assert_eq!(open_fd_count(), before);
}

#[test]
fn test_recvmsg_large_payload_chunked() {
    let (a, b) = socketpair().unwrap();
    let payload: Vec<u8> = (0..2 * BUF).map(|i| (i % 251) as u8).collect();
    let (c, _peer) = socketpair().unwrap();
    // `sendmsg` queues the whole 128 KiB atomically; the kernel splits it
    // into fragments, so `recvmsg` returns them one per call (sized by the
    // kernel, not by the read buffer). The fd rides the first fragment.
    sendmsg(&a, &payload, &[c]).unwrap();

    let mut got = Vec::new();
    let mut first = true;
    loop {
        let mut buf = buf();
        let msg = match recvmsg(&b, &mut buf, if first { 1 } else { 0 }, false) {
            Ok(m) => m,
            // All data was queued before the first read: EAGAIN ends the stream.
            Err(e) if matches!(e.current_context(), SyscallError::EAGAIN(_)) => break,
            Err(e) => panic!("unexpected error: {e:?}"),
        };
        if first {
            assert_eq!(msg.fds.len(), 1);
            first = false;
        } else {
            assert!(msg.fds.is_empty());
        }
        got.extend_from_slice(&buf[..msg.payload_len]);
    }
    assert_eq!(got, payload);
}

#[test]
fn test_recvmsg_creds() {
    // SO_PASSCRED is a receiver-side option: enable it on the reading end.
    let (a, b) = socketpair().unwrap();
    b.set_passcred().unwrap();
    sendmsg(&a, b"cred", &[]).unwrap();
    let mut buf = buf();
    let msg = recvmsg(&b, &mut buf, 0, true).unwrap();
    assert_eq!(&buf[..msg.payload_len], b"cred");
    let Some((pid, uid, gid)) = msg.cred else {
        panic!("expected a SCM_CREDENTIALS cmsg");
    };
    assert_eq!(pid, std::process::id() as i32);
    // SAFETY: getuid/getgid are always safe to call; they never fail.
    let (self_uid, self_gid) = unsafe { (libc::getuid(), libc::getgid()) };
    assert_eq!(uid, self_uid);
    assert_eq!(gid, self_gid);
}

#[test]
fn test_recvmsg_cred_requested_but_not_enabled() {
    // No SO_PASSCRED: no ucred cmsg arrives, `cred` stays None, no error.
    let (a, b) = socketpair().unwrap();
    sendmsg(&a, b"no-cred", &[]).unwrap();
    let mut buf = buf();
    let msg = recvmsg(&b, &mut buf, 0, true).unwrap();
    assert!(msg.cred.is_none());
}

#[test]
fn test_recvmsg_ctrunc_on_unreserved_cred_cmsg() {
    // SO_PASSCRED on the receiver makes the kernel attach a ucred cmsg to
    // every message. Without reserved space the kernel cannot deliver it and
    // sets MSG_CTRUNC, which the wrapper surfaces as E2BIG.
    let (a, b) = socketpair().unwrap();
    b.set_passcred().unwrap();
    sendmsg(&a, b"skip-cred", &[]).unwrap();
    let mut buf = buf();
    let err = recvmsg(&b, &mut buf, 0, false).unwrap_err();
    assert_eq!(err.current_context(), &SyscallError::E2BIG("recvmsg"));
}

#[test]
fn test_recvmsg_creds_with_fd() {
    // One fd plus creds in a single message: the control buffer must reserve
    // space for both cmsgs; a reservation short for the cred cmsg truncates it
    // and the legitimate receive fails with E2BIG.
    let (a, b) = socketpair().unwrap();
    let (c, peer) = socketpair().unwrap();
    b.set_passcred().unwrap();
    sendmsg(&a, b"cf", &[c]).unwrap();
    let mut buf = buf();
    let msg = recvmsg(&b, &mut buf, 1, true).unwrap();
    assert_eq!(&buf[..msg.payload_len], b"cf");
    assert_eq!(msg.fds.len(), 1);
    msg.fds[0].write(b"q").unwrap();
    let mut out = [0u8; 1];
    let n = peer.read(&mut out).unwrap();
    assert_eq!(&out[..n], b"q");
    let Some((pid, uid, gid)) = msg.cred else {
        panic!("expected a SCM_CREDENTIALS cmsg");
    };
    assert_eq!(pid, std::process::id() as i32);
    // SAFETY: getuid/getgid are always safe to call; they never fail.
    let (self_uid, self_gid) = unsafe { (libc::getuid(), libc::getgid()) };
    assert_eq!(uid, self_uid);
    assert_eq!(gid, self_gid);
}

#[test]
fn test_recvmsg_max_fds() {
    // 64 fds need the full `MAX_CTRL` control buffer; a smaller one truncates
    // the rights cmsg and the legitimate receive fails with E2BIG.
    let (a, b) = socketpair().unwrap();
    let mut fds = Vec::new();
    for _ in 0..64 {
        let (c, _peer) = socketpair().unwrap();
        fds.push(c);
    }
    sendmsg(&a, b"m", &fds).unwrap();
    let mut buf = buf();
    let msg = recvmsg(&b, &mut buf, 64, false).unwrap();
    assert_eq!(&buf[..msg.payload_len], b"m");
    assert_eq!(msg.fds.len(), 64);
}

#[test]
fn test_sendmsg_retries_eagain_until_fully_sent() {
    // A payload larger than the (shrunk) send buffer forces the send loop into
    // multiple `sendmsg` iterations: without the EAGAIN retry the first partial
    // send would abort with `EAGAIN`. The fd must ride exactly the first
    // fragment and the whole payload must arrive byte-exact.
    let (a, b) = socketpair().unwrap();
    let sndbuf: libc::c_int = 4096;
    // SAFETY: `a` is a valid socket; `sndbuf` is a valid `c_int` for SO_SNDBUF.
    let rc = unsafe {
        libc::setsockopt(
            a.as_raw(),
            libc::SOL_SOCKET,
            libc::SO_SNDBUF,
            (&raw const sndbuf).cast(),
            core::mem::size_of_val(&sndbuf) as libc::socklen_t,
        )
    };
    assert_eq!(rc, 0);
    let (c, _peer) = socketpair().unwrap();
    let payload: Vec<u8> = (0..256 * 1024).map(|i| (i % 251) as u8).collect();
    let total = payload.len();

    let reader = std::thread::spawn(move || {
        let mut got = Vec::new();
        let mut fd_frags = 0usize;
        loop {
            let mut buf = buf();
            let msg = match recvmsg(&b, &mut buf, 4, false) {
                Ok(m) => m,
                Err(e) if matches!(e.current_context(), SyscallError::EAGAIN(_)) => {
                    std::thread::yield_now();
                    continue;
                }
                Err(e) => panic!("unexpected error: {e:?}"),
            };
            if !msg.fds.is_empty() {
                fd_frags += 1;
            }
            got.extend_from_slice(&buf[..msg.payload_len]);
            if got.len() == total {
                break;
            }
        }
        (got, fd_frags)
    });
    // Let the reader start draining before the (blocking) send.
    std::thread::sleep(std::time::Duration::from_millis(50));
    sendmsg(&a, &payload, &[c]).unwrap();
    let (got, fd_frags) = reader.join().unwrap();
    assert_eq!(got, payload);
    assert_eq!(fd_frags, 1);
}
