//! E2E: the socket-lifecycle builtins `bind`, `listen`, `accept`.
//!
//! The test side plays the network peer (abstract or AF_INET), the shell
//! side plays the server. Abstract names are system-wide, so every address
//! is unique per test run.

#![allow(clippy::unwrap_used)]

use std::io::{BufRead, BufReader, Read, Write};
use std::os::linux::net::SocketAddrExt;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::{SocketAddr, UnixStream};
use std::process::{Child, Command, Stdio};
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// A unique abstract-socket name (the abstract namespace is system-wide).
fn uniq(tag: &str) -> String {
    format!(
        "fdshell-sock-{tag}-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    )
}

/// A pid-derived loopback port; `offset` keeps parallel tests apart.
fn inet_port(offset: u16) -> u16 {
    (10000u32 + std::process::id() % 20000 + u32::from(offset)) as u16
}

fn connect_abstract(name: &str) -> UnixStream {
    let addr = SocketAddr::from_abstract_name(name.as_bytes()).unwrap();
    UnixStream::connect_addr(&addr).unwrap()
}

fn run(script: &str) -> (String, String, i32) {
    let out = Command::new(BIN)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&out.stdout).unwrap().to_string(),
        str::from_utf8(&out.stderr).unwrap().to_string(),
        out.status.code().unwrap_or(-1),
    )
}

fn run_in(dir: &std::path::Path, script: &str) -> (String, String, i32) {
    let out = Command::new(BIN)
        .args(["-c", script])
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&out.stdout).unwrap().to_string(),
        str::from_utf8(&out.stderr).unwrap().to_string(),
        out.status.code().unwrap_or(-1),
    )
}

fn tmpdir(tag: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "fdshell-sock-{tag}_{}_{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Spawn a shell that serves on a unique abstract address until the test
/// side connects; the script is the server.
fn spawn_server(script: &str) -> Child {
    Command::new(BIN)
        .args(["-c", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

/// `bind @name %>%s`: success, `statx` agrees, and no filesystem object
/// exists (abstract namespace).
#[test]
fn bind_abstract_no_file_created() {
    let dir = tmpdir("abstract");
    let n = uniq("abs");
    let (out, err, code) = run_in(&dir, &format!("builtin bind @{n} %>%s; builtin statx %s"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.contains("kind=sock"), "stdout={out:?}");
    assert!(!dir.join(&n).exists());
    // A second bind to the same abstract name while the first is held:
    // EADDRINUSE (98) — the script's last status.
    let (out, err, code) = run_in(
        &dir,
        &format!("builtin bind @{n} %>%s1; builtin bind @{n} %>%s2"),
    );
    assert_eq!(code, 98, "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
}

/// `bind path %>%s`: the kernel resolves against the CWD and creates a
/// socket file; a stale path (left behind by an earlier shell) is
/// EADDRINUSE until unlinked; after unlink, rebind succeeds.
#[test]
fn bind_path_creates_socket_file() {
    let dir = tmpdir("path");
    let (out, err, code) = run_in(&dir, "builtin bind sock %>%s; builtin statx %s");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.contains("kind=sock"), "stdout={out:?}");
    let meta = std::fs::metadata(dir.join("sock")).unwrap();
    assert!(meta.file_type().is_socket());
    // The shell exited but the socket file remains: binding the same path
    // again is EADDRINUSE — the kernel's stale-path signal.
    let (out, err, code) = run_in(&dir, "builtin bind sock %>%s");
    assert_eq!(code, 98, "stdout={out:?} stderr={err:?}");
    // The script owns the path: unlink, then rebind succeeds.
    std::fs::remove_file(dir.join("sock")).unwrap();
    let (_out, err, code) = run_in(&dir, "builtin bind sock %>%s");
    assert_eq!(code, 0, "stderr={err:?}");
    let _ = std::fs::remove_file(dir.join("sock"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A second bind to the same path while the first fd var is held: the
/// script's last status is EADDRINUSE (98).
#[test]
fn bind_path_duplicate_while_held_is_eaddrinuse() {
    let dir = tmpdir("pathdup");
    let (out, err, code) = run_in(&dir, "builtin bind sock %>%s1; builtin bind sock %>%s2");
    assert_eq!(code, 98, "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    let _ = std::fs::remove_file(dir.join("sock"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `bind --bind ADDR --port N` binds a real AF_INET v4 endpoint: a UDP
/// probe to the port succeeds (no ICMP port-unreachable comes back) while
/// the shell holds the dgram socket. A stream connect is not used — a bound
/// socket is not listening, and the kernel refuses it by design.
#[test]
fn bind_inet_loopback() {
    let port = inet_port(0);
    let mut child = spawn_server(&format!(
        "builtin bind --type dgram --bind 127.0.0.1 --port {port} %>%s; sleep 1"
    ));
    std::thread::sleep(std::time::Duration::from_millis(500));
    let probe = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    probe.send_to(b"x", ("127.0.0.1", port)).unwrap();
    probe
        .set_read_timeout(Some(std::time::Duration::from_millis(500)))
        .unwrap();
    // The datagram was queued for the shell's socket: no ICMP
    // port-unreachable, so a timed recv just waits.
    let r = probe.recv_from(&mut [0u8; 1]);
    assert!(
        matches!(r, Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock),
        "port must be bound, got {r:?}"
    );
    let status = child.wait().unwrap();
    assert!(status.success());
}

/// A non-numeric `--bind` address is `inet_pton`'s EINVAL (exit 22).
#[test]
fn bind_inet_non_numeric_address_is_einval() {
    let (out, err, code) = run("builtin bind --bind notanip --port 1 %>%s");
    assert_eq!(code, 22, "stderr={err:?}");
    assert!(out.is_empty());
}

/// Usage errors are exit 1 with a one-line report on stderr.
#[test]
fn bind_parse_errors() {
    let long = "a".repeat(108);
    let cases = [
        (
            "builtin bind",
            "missing argument address",
            "Pass an @name, a path, or --bind ADDR --port N",
        ),
        (
            "builtin bind --type bogus x",
            "invalid argument type",
            "Use stream or dgram",
        ),
        (
            "builtin bind --bind 127.0.0.1",
            "invalid argument port",
            "Pass the port with --port N",
        ),
        (
            "builtin bind sock --bind 1.2.3.4 --port 1",
            "invalid argument address",
            "Use the positional ADDRESS or --bind/--port, not both",
        ),
        (
            &format!("builtin bind {long}"),
            "invalid argument address",
            "Address must be at most 107 bytes",
        ),
        (
            "builtin bind --bind 127.0.0.1 --port 65536",
            "invalid argument port",
            "Port must be a number in 0..=65535",
        ),
    ];
    for (script, msg, suggestion) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "script={script:?} stderr={err:?}");
        assert!(out.is_empty(), "stdout={out:?}");
        assert!(err.contains(msg), "script={script:?} stderr={err:?}");
        assert!(err.contains(suggestion), "script={script:?} stderr={err:?}");
    }
}

/// `listen` + `accept` stream round-trip over an abstract socket: the script
/// serves one line, the test side is the client.
#[test]
fn listen_accept_abstract_roundtrip() {
    let n = uniq("rt");
    let child = spawn_server(&format!(
        "builtin listen @{n} %>%s; echo ready; \
         builtin accept %s %>%c; read -u %c L; echo \"got:$L\""
    ));
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut client = connect_abstract(&n);
    client.write_all(b"hello\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={:?}",
        str::from_utf8(&out.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&out.stdout).unwrap(), "ready\ngot:hello\n");
}

/// The AF_INET variant: the test side connects over `std::net::TcpStream`.
#[test]
fn listen_accept_inet_roundtrip() {
    let port = inet_port(1);
    let child = spawn_server(&format!(
        "builtin listen --bind 127.0.0.1 --port {port} %>%s; echo ready; \
         builtin accept %s %>%c; read -u %c L; echo \"got:$L\""
    ));
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    client.write_all(b"hello\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={:?}",
        str::from_utf8(&out.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&out.stdout).unwrap(), "ready\ngot:hello\n");
}

/// `accept` on a bound (not listening) socket: EINVAL (22).
#[test]
fn accept_on_bound_not_listening_is_einval() {
    let n = uniq("nol");
    let (out, err, code) = run(&format!("builtin bind @{n} %>%s; builtin accept %s %>%c"));
    assert_eq!(code, 22, "stderr={err:?}");
    assert!(out.is_empty());
}

/// `listen` on a dgram socket: EOPNOTSUPP (95).
#[test]
fn listen_on_dgram_is_enotsupp() {
    let n = uniq("dgram");
    let (out, err, code) = run(&format!("builtin listen --type dgram @{n} %>%s"));
    assert_eq!(code, 95, "stderr={err:?}");
    assert!(out.is_empty());
}

/// `accept %nope %>%c`: no such fd var, exit 1 with the report.
#[test]
fn accept_unknown_fd_var() {
    let (out, err, code) = run("builtin accept %nope %>%c");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(
        err.contains("no fd variable with that name is set"),
        "stderr={err:?}"
    );
    assert!(out.is_empty());
}

/// Bounded capture is the accept-and-close cap: with `%>%conns[2]`, the
/// third connection is accepted by the child but never received by the
/// parent, so the kernel closes it (clean EOF) while conns[0..2] are held.
#[test]
fn bounded_capture_closes_overflow_connections() {
    let n = uniq("cap");
    let child = spawn_server(&format!(
        "builtin listen @{n} %>%s; echo ready; \
         builtin accept %s %>%conns[2]; \
         builtin accept %s %>%conns[2]; \
         builtin accept %s %>%conns[2]; \
         for %x in %conns; do echo conn; done; \
         sleep 2"
    ));
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut c1 = connect_abstract(&n);
    let c2 = connect_abstract(&n);
    let mut c3 = connect_abstract(&n);
    // c3 was never received: EOF while the shell still holds c1/c2.
    c3.set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    let mut buf = [0u8; 1];
    assert_eq!(c3.read(&mut buf).unwrap(), 0, "c3 should see EOF");
    // c1 is still open: a timed read sees neither data nor EOF.
    c1.set_read_timeout(Some(std::time::Duration::from_millis(300)))
        .unwrap();
    assert!(
        matches!(
            c1.read(&mut buf),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
        ),
        "c1 should still be open"
    );
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(str::from_utf8(&out.stdout).unwrap(), "ready\nconn\nconn\n");
    drop(c1);
    drop(c2);
}

/// Tagged bounded capture, the general form: `%accept>%conns[2]` only takes
/// fds sent with tag `accept` (from `builtin accept`) and caps `%conns` at
/// 2 entries in total. `builtin pipe` runs while the cap has room: its
/// `rd`/`wr` fds must stay out of `%conns`. The third accepted connection is
/// beyond the cap, so it is closed (clean EOF) while c1/c2 are held.
#[test]
fn tagged_bounded_capture_via_accept() {
    let n = uniq("captag");
    let child = spawn_server(&format!(
        "builtin listen @{n} %>%s; echo ready; \
         builtin pipe %accept>%conns[2] %rd>%r %wr>%w; echo piped; \
         builtin accept %s %accept>%conns[2]; \
         builtin accept %s %accept>%conns[2]; \
         builtin accept %s %accept>%conns[2]; \
         for %x in %conns; do echo conn; done; \
         sleep 2"
    ));
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut c1 = connect_abstract(&n);
    let c2 = connect_abstract(&n);
    let mut c3 = connect_abstract(&n);
    // c3 was beyond the cap: EOF while the shell still holds c1/c2.
    c3.set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    let mut buf = [0u8; 1];
    assert_eq!(c3.read(&mut buf).unwrap(), 0, "c3 should see EOF");
    // c1 is still open: a timed read sees neither data nor EOF.
    c1.set_read_timeout(Some(std::time::Duration::from_millis(300)))
        .unwrap();
    assert!(
        matches!(
            c1.read(&mut buf),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
        ),
        "c1 should still be open"
    );
    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={:?}",
        str::from_utf8(&out.stderr).unwrap()
    );
    // `conn` exactly twice: the pipe's rd/wr fds stayed out of `%conns`
    // (tag mismatch, cap had room), and the cap held at 2.
    assert_eq!(
        str::from_utf8(&out.stdout).unwrap(),
        "ready\npiped\nconn\nconn\n"
    );
    drop(c1);
    drop(c2);
}

/// Backgrounding is the non-blocking accept form: `&>&x` returns at once,
/// `waitpid &x` reaps + commits, and `%c` is usable.
#[test]
fn background_accept_commits_on_waitpid() {
    let n = uniq("bg");
    let mut child = spawn_server(&format!(
        "builtin listen @{n} %>%s; echo ready; \
         builtin accept %s %>%c &>&x; echo bg-started; \
         waitpid &x; builtin statx %c"
    ));
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    line.clear();
    stdout.read_line(&mut line).unwrap();
    assert_eq!(line, "bg-started\n");
    // The shell is blocked in `waitpid`; the accept child is blocked in
    // `accept`. Only now connect.
    let client = connect_abstract(&n);
    let mut rest = String::new();
    stdout.read_to_string(&mut rest).unwrap();
    let status = child.wait().unwrap();
    assert!(status.success(), "rest={rest:?}");
    assert!(rest.contains("kind=sock"), "rest={rest:?}");
    drop(client);
}

/// One-shot `wait` arm (previews the event-loop form, task #52): the
/// `readable` arm fires when a connection is queued, the arm accepts, reads
/// the line, and the `after` arm never runs.
#[test]
fn wait_arm_accept_one_shot() {
    let n = uniq("wait");
    let child = spawn_server(&format!(
        "builtin listen @{n} %>%s; echo ready; \
         wait readable %s %>%conns[1]) builtin accept %s %>%c; read -u %c L; echo \"got:$L\" ;; \
         after 3000) echo TIMEOUT ;; done"
    ));
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut client = connect_abstract(&n);
    client.write_all(b"hi\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={:?}",
        str::from_utf8(&out.stderr).unwrap()
    );
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(stdout.contains("got:hi"), "stdout={stdout:?}");
    assert!(!stdout.contains("TIMEOUT"), "stdout={stdout:?}");
    drop(client);
}
