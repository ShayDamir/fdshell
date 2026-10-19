//! E2E: `recvmsg` — cred surfacing (`PID:UID:GID`), the `wait`-arm readiness
//! pattern, EOF, fd-count mismatch and NUL payloads. The socket pair is the
//! same as in `tests/sendmsg.rs`: each end is a shell's stdin (fd `0`).

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

fn output(child: Child) -> (String, String, i32) {
    let out = child.wait_with_output().unwrap();
    (
        str::from_utf8(&out.stdout).unwrap().to_string(),
        str::from_utf8(&out.stderr).unwrap().to_string(),
        out.status.code().unwrap_or(-1),
    )
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). The pid-named temp files live inside this dir now. Removal is
/// `.ok()`-tolerant: teardown never panics, and it runs on a failing test's
/// unwind, which is exactly the test that used to leak.
struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl std::ops::Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A pid + counter scratch dir `fdshell-recvmsg-<tag>-<pid>-<counter>`;
/// returns the path of a `.txt` file in it plus the guard — bind both.
fn temp_path(tag: &str) -> (String, Scratch) {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fdshell-recvmsg-{tag}-{}-{}",
        std::process::id(),
        c
    ));
    std::fs::create_dir_all(&dir).unwrap();
    (
        dir.join(format!("{tag}.txt")).to_str().unwrap().to_string(),
        Scratch(dir),
    )
}

/// Real uid/gid of this user from `/proc/self/status` (the sender is our
/// child, so it shares both).
fn own_uid_gid() -> (u32, u32) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let field = |line: &str| line.split_whitespace().nth(1).unwrap().parse().unwrap();
    let uid = status
        .lines()
        .find(|l| l.starts_with("Uid:"))
        .map(field)
        .unwrap();
    let gid = status
        .lines()
        .find(|l| l.starts_with("Gid:"))
        .map(field)
        .unwrap();
    (uid, gid)
}

/// `--cred VAR` stores `PID:UID:GID` of the sending process — the pid is the
/// one the script can check against.
#[test]
fn cred_captures_sender_identity() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();

    let a = spawn(
        a_sock,
        "recvmsg --cred C 0 MSG; echo \"CRED=$C\"; echo \"MSG=$MSG\"",
    );
    // `--passcred` enables `SO_PASSCRED` on the sender's socket before the
    // send: the kernel captures the real credentials at send time, so the
    // result is deterministic even though B exits right after sending.
    let mut b = spawn(b_sock, "sendmsg 0 --passcred --msg ping");
    let b_pid = b.id() as i32;
    let (out, err, _code) = output(a);
    let _ = b.wait();

    let (uid, gid) = own_uid_gid();
    let expected = format!("CRED={b_pid}:{uid}:{gid}");
    assert!(out.contains(&expected), "stdout={out:?} stderr={err:?}");
    assert!(out.contains("MSG=ping"), "stdout={out:?}");
}

/// The headline readiness pattern: `import_fd` brings the inherited socket in
/// as an fd var, a `wait` readable arm blocks until the peer sends, and the
/// arm's `recvmsg` delivers payload and fd var together.
#[test]
fn wait_readable_arm_receives_after_delay() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let (path, _scratch) = temp_path("wait");
    std::fs::write(&path, b"").unwrap();

    let a = spawn(
        a_sock,
        "builtin import_fd 0 %>%sock; \
         wait readable %sock) recvmsg %sock VAR %f2; echo \"got $VAR\"; echo data >%f2 ;; \
         after 2000) echo TIMEOUT ;; done",
    );
    // Let A set up before B sends, so the wait arm (not a race) wins.
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut b = spawn(
        b_sock,
        &format!("builtin openat2 --flags O_RDWR {path} %>%f; sendmsg 0 --msg ready --fd %f"),
    );
    let (out, err, _code) = output(a);
    let _ = b.wait();

    assert!(out.contains("got ready"), "stdout={out:?} stderr={err:?}");
    assert!(!out.contains("TIMEOUT"), "stdout={out:?}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "data\n");
}

/// Peer closed the socket: the var is set empty and the command exits 1.
#[test]
fn eof_sets_empty_var_and_fails() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let a = spawn(a_sock, "recvmsg 0 MSG || echo eof; echo \"M=[$MSG]\"");
    let mut b = spawn(b_sock, "true");
    let (out, err, code) = output(a);
    let _ = b.wait();

    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "eof\nM=[]\n", "stderr={err:?}");
}

/// Declared slots more than the peer sent is a hard error: like every
/// intercept failure it aborts the script with a clean message (the payload
/// arrived, but atomicity means no vars are committed).
#[test]
fn fd_count_mismatch_is_hard_error() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let a = spawn(a_sock, "recvmsg 0 MSG %f2; echo UNREACHED");
    let mut b = spawn(b_sock, "sendmsg 0 --msg ping");
    let (out, err, code) = output(a);
    let _ = b.wait();

    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("fds"), "stderr={err:?}");
    assert!(!out.contains("UNREACHED"), "stdout={out:?}");
}

/// The peer sends more fds than the script declared: the kernel truncates
/// the control buffer, the error names it, and (as with every intercept
/// failure) the script aborts with no vars committed.
#[test]
fn more_fds_than_declared_is_hard_error() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let a = spawn(a_sock, "recvmsg 0 MSG %f9; echo UNREACHED");
    let mut b = spawn(
        b_sock,
        "builtin openat2 --flags O_RDWR /dev/null %>%f1; \
         builtin openat2 --flags O_RDWR /dev/null %>%f2; \
         sendmsg 0 --msg x --fd %f1 --fd %f2",
    );
    let (out, err, code) = output(a);
    let _ = b.wait();

    assert_eq!(code, 1, "stderr={err:?}");
    assert!(
        err.contains("the peer sent more fds than declared"),
        "stderr={err:?}"
    );
    assert!(!out.contains("UNREACHED"), "stdout={out:?}");
}

/// A NUL byte in the payload is a hard error (payloads are strings).
#[test]
fn nul_payload_is_hard_error() {
    let (a_sock, b_sock) = UnixStream::pair().unwrap();
    let a = spawn(a_sock, "recvmsg 0 MSG; echo UNREACHED");
    let mut b = spawn(b_sock, "builtin printf \"a\\000b\" >&0");
    let (out, err, code) = output(a);
    let _ = b.wait();

    assert_eq!(code, 1, "stdout={out:?}");
    assert!(err.contains("NUL"), "stderr={err:?}");
    assert!(!out.contains("UNREACHED"), "stdout={out:?}");
}
