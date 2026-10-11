#![allow(clippy::unwrap_used)]

use std::io::Read;
use std::ops::Deref;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A scratch dir for the `cat`-a-big-file case; removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("fdshell-sigpipe-{}-{}", std::process::id(), c));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn run(dir: &Path, script: &str) -> Output {
    Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .output()
        .unwrap()
}

/// Run `script` and assert bash's broken-pipe outcome: exit `code`, stdout
/// `stdout`, and **no** stderr — a stage that writes to a pipe with no
/// readers is killed by `SIGPIPE`, silently, and never prints a write error.
fn assert_silent(dir: &Path, script: &str, code: i32, stdout: &str) {
    let out = run(dir, script);
    assert_eq!(
        out.status.code(),
        Some(code),
        "`{script}`: stderr={:?}",
        str::from_utf8(&out.stderr)
    );
    assert_eq!(
        str::from_utf8(&out.stdout).unwrap(),
        stdout,
        "`{script}`: stderr={:?}",
        str::from_utf8(&out.stderr)
    );
    assert_eq!(
        str::from_utf8(&out.stderr).unwrap(),
        "",
        "`{script}` must be silent: a broken pipe kills the writer"
    );
}

/// Spawn `script` with piped stdout, read 4 KiB of it, close the read end, and
/// return the shell's `(status, stderr)`.
///
/// Deterministic by construction: every writer here emits far more than the
/// 64 KiB pipe capacity (`seq 1000000` is ~6.9 MB), so the stage is *guaranteed*
/// to still be blocked in `write` when the reader closes — no interleaving
/// makes the assertion a coin flip.
fn run_with_closed_stdout(dir: &Path, script: &str) -> (ExitStatus, String) {
    let mut child = Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut buf = [0u8; 4096];
    stdout.read_exact(&mut buf).unwrap();
    drop(stdout); // the read end closes while the writer is still writing
    let status = child.wait().unwrap();
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    (status, stderr)
}

/// An external writer whose downstream stage exits first dies of `SIGPIPE`:
/// no `seq: write error: Broken pipe`, and the pipeline status is the last
/// stage's (0).
#[test]
fn external_writer_dies_silently() {
    let dir = Scratch::new();
    assert_silent(&dir, "seq 1000000 | echo got", 0, "got\n");
}

/// fdshell's *own* write path: the `printf` builtin builds 128 KiB in memory
/// and writes it with `ImportedFd::write_all`, so it is the builtin that must
/// die of the signal (pre-fix it printed a `builtin execution failed` report).
#[test]
fn builtin_writer_dies_silently() {
    let dir = Scratch::new();
    // 65 536 words is the brace-expansion cap (`MAX_WORDS`); 70 000 is a
    // parse error, so there is no margin to give here.
    assert_silent(&dir, "printf \"xx%.0s\" {1..65536} | echo got", 0, "got\n");
}

/// An unbounded producer (`yes`) is silenced the same way.
#[test]
fn infinite_producer_is_silent() {
    let dir = Scratch::new();
    assert_silent(&dir, "yes | echo got", 0, "got\n");
}

/// The `cat | true` shape of the report: 200 KiB through a stage that reads
/// nothing and exits at once.
#[test]
fn cat_file_to_true_is_silent() {
    let dir = Scratch::new();
    std::fs::write(dir.join("big"), vec![b'x'; 200 * 1024]).unwrap();
    assert_silent(&dir, "cat big | true", 0, "");
}

/// The exact reported symptom, repeated: two builtin stages, the writer
/// racing the reader. Pre-fix this printed `echo: write error: Broken pipe`
/// in a third of the runs; post-fix every interleaving is silent, because
/// stage 0 either writes cleanly or dies of the signal.
#[test]
fn reported_repeat_is_always_silent() {
    let dir = Scratch::new();
    for i in 0..50 {
        let out = run(&dir, "echo a | echo b");
        assert_eq!(
            out.status.code(),
            Some(0),
            "run {i}: stderr={:?}",
            str::from_utf8(&out.stderr)
        );
        assert_eq!(
            str::from_utf8(&out.stdout).unwrap(),
            "b\n",
            "run {i}: stderr={:?}",
            str::from_utf8(&out.stderr)
        );
        assert_eq!(
            str::from_utf8(&out.stderr).unwrap(),
            "",
            "run {i} printed a broken-pipe error"
        );
    }
}

/// The pipeline's status is the last stage's, and a stage killed by `SIGPIPE`
/// is 141 (`128 + 13`): `seq 1000000 | cat` with its stdout closed mid-run.
#[test]
fn last_stage_sigpipe_status_is_141() {
    let dir = Scratch::new();
    let (status, stderr) = run_with_closed_stdout(&dir, "seq 1000000 | cat");
    assert_eq!(status.code(), Some(141), "stderr={stderr:?}");
    assert_eq!(stderr, "", "closed stdout must not print a write error");
}

/// `exec`/`become` replace the shell's own image, so the program must not
/// inherit the shell's `SIG_IGN` for `SIGPIPE` (Rust std installs it). bash
/// gives the same shape: the process is *signaled* 13, not exiting 141.
#[test]
fn exec_handoff_dies_on_sigpipe() {
    let dir = Scratch::new();
    let (status, stderr) = run_with_closed_stdout(&dir, "exec seq 1000000");
    assert_eq!(status.signal(), Some(13), "stderr={stderr:?}");
    assert_eq!(stderr, "", "exec'd program must die silently");
}

/// `signalfd %v PIPE` blocks `SIGPIPE` in the shell (that is how a signalfd
/// receives it), and `clone3` inherits the mask — so the hand-off to a stage
/// must clear the mask as well as the disposition, or the stage's write still
/// fails with `EPIPE` and prints the noise.
#[test]
fn signalfd_blocked_sigpipe_is_unblocked_in_stages() {
    let dir = Scratch::new();
    assert_silent(&dir, "signalfd %p PIPE; seq 1000000 | echo got", 0, "got\n");
}
