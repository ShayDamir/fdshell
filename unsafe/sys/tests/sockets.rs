//! Integration tests for the socket-lifecycle wrappers
//! (`sys::net::{socket, bind_uds_path, bind_uds_abstract, bind_inet,
//! connect_uds_path, connect_uds_abstract, connect_inet, listen, accept,
//! getsockname}`).

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::io::{Read, Write};
use std::ops::Deref;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::{SocketAddr, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). Removal is `.ok()`-tolerant: teardown never panics, and it runs
/// on a failing test's unwind, which is exactly the test that used to leak.
/// `sock_path` keeps the guarded prefix short (`fdshell-sockets-<tag>-<pid>-<c>`)
/// so the bound `…/sock` stays well under `sun_path`'s 108 bytes.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A unique abstract-namespace name: tests in this binary share one pid, so
/// the counter keeps parallel tests apart; the pid keeps binaries apart.
fn abs_name(tag: &str) -> String {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("fdshell-{tag}-{}-{c}", std::process::id())
}

/// A pid + counter scratch dir named after `tag` (the old fixed prefixes
/// `fdshell-sockets-<tag>-…`), removed when the test's guard drops.
fn scratch(tag: &str) -> Scratch {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fdshell-sockets-{tag}-{}-{}",
        std::process::id(),
        c
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
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
    let dir = scratch("path");
    let sock_path = dir.join("sock");
    let fd = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    sys::net::bind_uds_path(&fd, cstr(sock_path.to_str().unwrap())).unwrap();
    let meta = std::fs::metadata(&sock_path).unwrap();
    assert!(
        meta.file_type().is_socket(),
        "bind must create a socket file"
    );
}

#[test]
fn test_bind_inet_loopback_port_zero() {
    let fd = sys::net::socket(libc::AF_INET, libc::SOCK_STREAM).unwrap();
    sys::net::bind_inet(&fd, cstr("127.0.0.1"), 0).unwrap();
    // Port 0 asks the kernel for a port; `getsockname` must report the one
    // it assigned (task #254: the read-back is what lets a test bind 0).
    match sys::net::getsockname(&fd).unwrap() {
        sys::net::SockName::Inet { port } => assert_ne!(port, 0),
        other => panic!("AF_INET socket must report an Inet name, got {other:?}"),
    }
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

#[test]
fn test_connect_uds_abstract_roundtrip() {
    let name = abs_name("connect-abs");
    let listener = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    sys::net::bind_uds_abstract(&listener, cstr(&name)).unwrap();
    sys::net::listen(&listener, 1).unwrap();

    let client = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    sys::net::connect_uds_abstract(&client, cstr(&name)).unwrap();

    let conn = sys::net::accept(&listener).unwrap();
    conn.verify().unwrap();

    // The client's `connect` used the exact addrlen the kernel stored at
    // `bind`; a round-trip proves the two match end to end.
    client.write_all(b"hi\n").unwrap();
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
fn test_connect_uds_path_roundtrip() {
    let dir = scratch("connect-path");
    let sock_path = dir.join("sock");
    let listener = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    sys::net::bind_uds_path(&listener, cstr(sock_path.to_str().unwrap())).unwrap();
    sys::net::listen(&listener, 1).unwrap();

    let client = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    sys::net::connect_uds_path(&client, cstr(sock_path.to_str().unwrap())).unwrap();
    let conn = sys::net::accept(&listener).unwrap();
    conn.verify().unwrap();
    client.write_all(b"ok\n").unwrap();
    let mut buf = [0u8; 3];
    let mut got = 0;
    while got < buf.len() {
        let n = conn.read(&mut buf[got..]).unwrap();
        assert!(n > 0, "accepted fd must stay readable");
        got += n;
    }
    assert_eq!(&buf, b"ok\n");
}

/// `connect` never creates a socket file: a path nobody bound is `ENOENT`.
#[test]
fn test_connect_uds_path_to_unbound_path_is_enoent() {
    let dir = scratch("connect-missing");
    let missing = dir.join("never-bound");
    let fd = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    let err = sys::net::connect_uds_path(&fd, cstr(missing.to_str().unwrap())).unwrap_err();
    assert_eq!(err, sys::SyscallError::ENOENT(UNKNOWN));
}

#[test]
fn test_connect_uds_abstract_max_length() {
    // 107 bytes fit `sun_path` with the leading NUL: the wrapper lets it
    // through to the kernel (nothing is bound, so `ECONNREFUSED` — not the
    // wrapper's `EINVAL`). 108 bytes are rejected by the wrapper itself, so
    // the error carries the `call` argument, "connect".
    let fd = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    let ok = "a".repeat(107);
    let err = sys::net::connect_uds_abstract(&fd, cstr(&ok)).unwrap_err();
    assert_eq!(
        err,
        sys::SyscallError::Other {
            errno: libc::ECONNREFUSED,
            syscall: UNKNOWN
        }
    );
    let long = "b".repeat(108);
    let err = sys::net::connect_uds_abstract(&fd, cstr(&long)).unwrap_err();
    assert_eq!(err, sys::SyscallError::EINVAL("connect"));
}

#[test]
fn test_connect_inet_loopback() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();

    let client = sys::net::socket(libc::AF_INET, libc::SOCK_STREAM).unwrap();
    sys::net::connect_inet(&client, cstr("127.0.0.1"), port).unwrap();

    let (mut conn, _) = listener.accept().unwrap();
    client.write_all(b"hi\n").unwrap();
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
fn test_connect_inet_non_numeric_is_einval() {
    let fd = sys::net::socket(libc::AF_INET, libc::SOCK_STREAM).unwrap();
    let err = sys::net::connect_inet(&fd, cstr("not-an-ip"), 8080).unwrap_err();
    assert_eq!(err, sys::SyscallError::EINVAL("inet_pton"));
}

#[test]
fn test_connect_refused_when_nothing_bound() {
    let name = abs_name("refused");
    let fd = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    let err = sys::net::connect_uds_abstract(&fd, cstr(&name)).unwrap_err();
    assert_eq!(
        err,
        sys::SyscallError::Other {
            errno: libc::ECONNREFUSED,
            syscall: UNKNOWN
        }
    );
}

/// `getsockname` read-back of an AF_INET `port 0` bind is the live endpoint
/// in host byte order: a real client connects to the reported port, so a
/// byte-swapped (`from_le`) read-back lands on a port nothing listens on
/// and the connect fails. (Binding a second socket to the reported port to
/// pin byte order is not possible — the kernel-assigned port is live on the
/// first socket, so the second `bind` is `EADDRINUSE`.)
#[test]
fn test_getsockname_inet_reports_bound_port() {
    let fd = sys::net::socket(libc::AF_INET, libc::SOCK_STREAM).unwrap();
    sys::net::bind_inet(&fd, cstr("127.0.0.1"), 0).unwrap();
    sys::net::listen(&fd, 1).unwrap();
    let port = match sys::net::getsockname(&fd).unwrap() {
        sys::net::SockName::Inet { port } => port,
        other => panic!("AF_INET socket must report an Inet name, got {other:?}"),
    };
    assert_ne!(port, 0, "port 0 must come back kernel-assigned");
    let client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    drop(client);
}

/// Abstract form: the read-back is the name without its leading NUL, so
/// this pins the family dispatch, the `sun_path` offset and the
/// `addrlen`-bounded slice (abstract names are length-significant).
#[test]
fn test_getsockname_uds_abstract_reports_name() {
    let name = abs_name("gsn-abs");
    let (fd, _peer) = sys::net::socketpair().unwrap();
    sys::net::bind_uds_abstract(&fd, cstr(&name)).unwrap();
    match sys::net::getsockname(&fd).unwrap() {
        sys::net::SockName::Unix { name: got } => assert_eq!(got, name.as_bytes()),
        other => panic!("AF_UNIX socket must report a Unix name, got {other:?}"),
    }
}

/// Path form: the read-back is the bound path truncated at the terminating
/// NUL (the kernel pads the full `sockaddr_un`). pid+counter dir per the
/// LESSONS temp-path rule, tolerant teardown.
#[test]
fn test_getsockname_uds_path_reports_name() {
    let dir = scratch("gsn-path");
    let sock_path = dir.join("sock");
    let fd = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    sys::net::bind_uds_path(&fd, cstr(sock_path.to_str().unwrap())).unwrap();
    match sys::net::getsockname(&fd).unwrap() {
        sys::net::SockName::Unix { name: got } => {
            assert_eq!(got, sock_path.as_os_str().as_bytes())
        }
        other => panic!("AF_UNIX socket must report a Unix name, got {other:?}"),
    }
}

/// A non-socket fd surfaces the kernel errno through the wrapper; `cvt`
/// carries no syscall name, so it appears as `UNKNOWN` (see above). The
/// bound path socket beside the memfd makes the contrast explicit: same
/// call, `Ok` on a bound socket, `ENOTSOCK` on a plain fd.
#[test]
fn test_getsockname_on_non_socket_is_enotsock() {
    let dir = scratch("gsn-notsock");
    let sock_path = dir.join("sock");
    let sock = sys::net::socket(libc::AF_UNIX, libc::SOCK_STREAM).unwrap();
    sys::net::bind_uds_path(&sock, cstr(sock_path.to_str().unwrap())).unwrap();
    sys::net::getsockname(&sock).unwrap();
    let memfd = sys::memfd::memfd_create().unwrap();
    let err = sys::net::getsockname(&memfd).unwrap_err();
    assert_eq!(
        err,
        sys::SyscallError::Other {
            errno: libc::ENOTSOCK,
            syscall: UNKNOWN
        }
    );
}

/// A family beyond AF_UNIX/AF_INET v4 is rejected by name (task #254): the
/// read-back is defined for the two families this wrapper decodes. The
/// socket is unbound and names no port, so the pin stays collision-free
/// under concurrent suite runs.
#[test]
fn test_getsockname_on_other_family_is_einval() {
    let fd = sys::net::socket(libc::AF_INET6, libc::SOCK_STREAM).unwrap();
    let err = sys::net::getsockname(&fd).unwrap_err();
    assert_eq!(err, sys::SyscallError::EINVAL("getsockname"));
}
