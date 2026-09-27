//! E2E: `sendmsg`/`recvmsg` move a byte payload plus fd vars over a
//! pre-connected AF_UNIX stream socket pair, each end wired in as a shell's
//! stdin (socket = raw fd `0`).

#![allow(clippy::unwrap_used)]

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn spawn(stdin: UnixStream, script: &str) -> Child {
    let fd: OwnedFd = stdin.into();
    Command::new(BIN)
        .args(["-c", script])
        .stdin(Stdio::from(fd))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

fn output(child: Child) -> (String, String) {
    let out = child.wait_with_output().unwrap();
    (
        str::from_utf8(&out.stdout).unwrap().to_string(),
        str::from_utf8(&out.stderr).unwrap().to_string(),
    )
}

fn temp_path(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!("sendmsg_{tag}_{}.txt", std::process::id()));
    path.to_str().unwrap().to_string()
}

/// The payload arrives as a string var and the fd var is a usable dup of the
/// sender's open file: A writes through it and B's file picks up the data.
#[test]
fn payload_and_fd_roundtrip() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let path = temp_path("roundtrip");
    std::fs::write(&path, b"").unwrap();

    let a = spawn(a_sock, "recvmsg 0 MSG %f2; echo \"$MSG\"; echo data >%f2");
    let mut b = spawn(
        b_sock,
        &format!("builtin openat2 --flags O_RDWR {path} %>%f; sendmsg 0 --msg hello --fd %f"),
    );
    let (out, err) = output(a);
    let _ = b.wait();

    assert_eq!(out, "hello\n", "stderr={err:?}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "data\n");
    let _ = std::fs::remove_file(&path);
}

/// `--msgfd %in N` sends the first N bytes read from an fd var as the payload.
#[test]
fn msgfd_payload() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let in_path = temp_path("msgfd_in");
    std::fs::write(&in_path, b"abcdef\n").unwrap();

    let a = spawn(a_sock, "recvmsg 0 MSG; echo \"$MSG\"");
    let mut b = spawn(
        b_sock,
        &format!("builtin openat2 --flags O_RDONLY {in_path} %>%in; sendmsg 0 --msgfd %in 4"),
    );
    let (out, err) = output(a);
    let _ = b.wait();

    assert_eq!(out, "abcd\n", "stderr={err:?}");
    let _ = std::fs::remove_file(&in_path);
}

/// A 2 KiB payload round-trips intact — larger than the 1088 B a
/// mis-sized single-receive buffer could hold, so truncation is caught.
#[test]
fn payload_larger_than_one_kib() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let in_path = temp_path("big");
    let data = "a".repeat(2048);
    std::fs::write(&in_path, data.as_bytes()).unwrap();

    let a = spawn(a_sock, "recvmsg 0 MSG; echo \"$MSG\"");
    let mut b = spawn(
        b_sock,
        &format!("builtin openat2 --flags O_RDONLY {in_path} %>%in; sendmsg 0 --msgfd %in 2048"),
    );
    let (out, err) = output(a);
    let _ = b.wait();

    assert_eq!(out, format!("{data}\n"), "stderr={err:?}");
    let _ = std::fs::remove_file(&in_path);
}

/// Repeated `--fd` vars arrive in declared order, each usable.
#[test]
fn multiple_fd_vars_arrive_in_order() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let p1 = temp_path("multi_1");
    let p2 = temp_path("multi_2");
    std::fs::write(&p1, b"").unwrap();
    std::fs::write(&p2, b"").unwrap();

    let a = spawn(
        a_sock,
        "recvmsg 0 MSG %f1 %f2; echo \"$MSG\"; echo one >%f1; echo two >%f2",
    );
    let mut b = spawn(
        b_sock,
        &format!(
            "builtin openat2 --flags O_RDWR {p1} %>%fa; \
             builtin openat2 --flags O_RDWR {p2} %>%fb; \
             sendmsg 0 --msg two --fd %fa --fd %fb"
        ),
    );
    let (out, err) = output(a);
    let _ = b.wait();

    assert_eq!(out, "two\n", "stderr={err:?}");
    assert_eq!(std::fs::read_to_string(&p1).unwrap(), "one\n");
    assert_eq!(std::fs::read_to_string(&p2).unwrap(), "two\n");
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
}

/// `sendmsg` with an unknown flag is a clean error (exit 1).
#[test]
fn unknown_flag_errors() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let a = spawn(a_sock, "true");
    let b = spawn(b_sock, "sendmsg 0 --bogus");
    let _ = output(a);
    let (_out, err) = output(b);
    assert!(err.contains("sendmsg"), "stderr={err:?}");
}

/// An empty payload cannot carry `--fd` vars: the kernel would silently drop
/// them on a stream socket, so the send is refused.
#[test]
fn empty_payload_with_fds_errors() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let a = spawn(a_sock, "true");
    let b = spawn(
        b_sock,
        "builtin openat2 --flags O_RDWR /dev/null %>%f; sendmsg 0 --fd %f",
    );
    let _ = output(a);
    let (_out, err) = output(b);
    assert!(
        err.contains("an empty payload cannot carry --fd vars"),
        "stderr={err:?}"
    );
}

/// A bare `sendmsg` (no payload form, no `--fd`) is a zero-byte no-op that
/// succeeds: it is the readiness-signaling form. A holds the peer open — a
/// 0-byte send to a closed peer is EPIPE, like any write after EOF.
#[test]
fn bare_sendmsg_is_noop() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let mut a = spawn(a_sock, "read X");
    let b = spawn(b_sock, "sendmsg 0 && echo ok");
    let (out, err) = output(b);
    assert_eq!(out, "ok\n", "stderr={err:?}");
    let _ = a.kill();
    let _ = a.wait();
}

/// More than 64 `--fd` vars is refused at the syscall layer and mapped to
/// the exact "at most 64" error.
#[test]
fn sixty_five_fd_vars_error() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let a = spawn(a_sock, "true");
    let mut script = String::new();
    for i in 0..65 {
        script.push_str(&format!(
            "builtin openat2 --flags O_RDWR /dev/null %>%f{i}; "
        ));
    }
    let mut fd_args = String::new();
    for i in 0..65 {
        fd_args.push_str(&format!("--fd %f{i} "));
    }
    script.push_str(&format!("sendmsg 0 --msg x {fd_args}"));
    let b = spawn(b_sock, &script);
    let _ = output(a);
    let (_out, err) = output(b);
    assert!(
        err.contains("at most 64 --fd vars per message"),
        "stderr={err:?}"
    );
}
