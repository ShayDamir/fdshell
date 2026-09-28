//! Integration tests for the socket-lifecycle wrappers
//! (`sys::net::{socket, bind_uds_path, bind_uds_abstract, bind_inet,
//! listen, accept}`).

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::io::Write;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::{SocketAddr, UnixStream};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A unique abstract-namespace name: tests in this binary share one pid, so
/// the counter keeps parallel tests apart; the pid keeps binaries apart.
fn abs_name(tag: &str) -> String {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("fdshell-{tag}-{}-{c}", std::process::id())
}

fn cstr(s: &str) -> std::ffi::CString {
    std::ffi::CString::new(s).unwrap()
}

/// `cvt` carries no syscall name, so kernel errnos surface as "unknown".
const UNKNOWN: &str = "unknown";

#[test]
fn test_socket_sets_cloexec() {
    let fd = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    fd.verify().unwrap();
}

#[test]
fn test_socket_dgram() {
    let fd = sys::net::socket(libc::AF_UNIX, libc::SOCK_DGRAM).unwrap();
    fd.verify().unwrap();
}

#[test]
fn test_bind_uds_abstract_and_duplicate() {
    let name = abs_name("dup");
    let a = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    sys::net::bind_uds_abstract(&a, cstr(&name)).unwrap();
    // Bind the exact same name a second time: EADDRINUSE.
    let b = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    let err = sys::net::bind_uds_abstract(&b, cstr(&name)).unwrap_err();
    assert_eq!(
        err,
        sys::SyscallError::Other {
            errno: libc::EADDRINUSE,
            syscall: UNKNOWN
        }
    );
}

#[test]
fn test_bind_uds_abstract_max_length() {
    // 107 bytes fit `sun_path` with the leading NUL; 108 do not.
    let ok = "a".repeat(107);
    let fd = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    sys::net::bind_uds_abstract(&fd, cstr(&ok)).unwrap();
    let long = "b".repeat(108);
    let err = sys::net::bind_uds_abstract(&fd, cstr(&long)).unwrap_err();
    // The wrapper rejects the over-long name itself, so it carries "bind".
    assert_eq!(err, sys::SyscallError::EINVAL("bind"));
}

#[test]
fn test_bind_uds_path_creates_file() {
    let dir = std::env::temp_dir().join(format!(
        "fdshell-sockets-path-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let sock_path = dir.join("sock");
    let fd = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    sys::net::bind_uds_path(&fd, cstr(sock_path.to_str().unwrap())).unwrap();
    let meta = std::fs::metadata(&sock_path).unwrap();
    assert!(
        meta.file_type().is_socket(),
        "bind must create a socket file"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_bind_inet_loopback_port_zero() {
    let fd = sys::net::socket(libc::AF_INET, libc::SOCK_STREAM).unwrap();
    sys::net::bind_inet(&fd, cstr("127.0.0.1"), 0).unwrap();
}

#[test]
fn test_bind_inet_non_numeric_is_einval() {
    let fd = sys::net::socket(libc::AF_INET, libc::SOCK_STREAM).unwrap();
    let err = sys::net::bind_inet(&fd, cstr("not-an-ip"), 8080).unwrap_err();
    assert_eq!(err, sys::SyscallError::EINVAL("inet_pton"));
}

#[test]
fn test_listen_on_dgram_is_unsupported() {
    let fd = sys::net::socket(libc::AF_UNIX, libc::SOCK_DGRAM).unwrap();
    let err = sys::net::listen(&fd, 1).unwrap_err();
    assert_eq!(
        err,
        sys::SyscallError::Other {
            errno: libc::EOPNOTSUPP,
            syscall: UNKNOWN
        }
    );
}

#[test]
fn test_accept_roundtrip() {
    let name = abs_name("accept");
    let listener = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    sys::net::bind_uds_abstract(&listener, cstr(&name)).unwrap();
    sys::net::listen(&listener, 1).unwrap();

    let client = SocketAddr::from_abstract_name(name.as_bytes()).unwrap();
    let mut peer = UnixStream::connect_addr(&client).unwrap();

    let conn = sys::net::accept(&listener).unwrap();
    conn.verify().unwrap();

    // The accepted fd is a usable connection: a byte written by the std
    // client arrives on it.
    peer.write_all(b"hi\n").unwrap();
    let mut buf = [0u8; 3];
    let mut got = 0;
    while got < buf.len() {
        let n = conn.read(&mut buf[got..]).unwrap();
        assert!(n > 0, "accepted fd must stay readable");
        got += n;
    }
    assert_eq!(&buf, b"hi\n");
}

#[test]
fn test_accept_on_non_listening_is_einval() {
    let name = abs_name("nolisten");
    let fd = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    sys::net::bind_uds_abstract(&fd, cstr(&name)).unwrap();
    let err = match sys::net::accept(&fd) {
        Ok(_) => panic!("accept on a non-listening socket must fail"),
        Err(e) => e,
    };
    assert_eq!(err, sys::SyscallError::EINVAL(UNKNOWN));
}
