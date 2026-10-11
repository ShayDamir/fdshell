#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Per-test scratch dir, removed on drop. Named with pid + an atomic counter:
/// tests in one binary share the process and run on parallel threads, so a
/// pid-only name would collide (LESSONS).
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let c = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("fdshell-mkfifoat-{}-{}", std::process::id(), c));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl std::ops::Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn run(dir: &std::path::Path, script: &str) -> Output {
    Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    str::from_utf8(&out.stdout).unwrap().to_string()
}

fn stderr(out: &Output) -> String {
    str::from_utf8(&out.stderr).unwrap().to_string()
}

/// Create the fifo inside the CWD via `--dirfd %CWD` and re-stat the handle:
/// `statx %ch` reports a mode-0600 fifo (rc 0 proves the capture committed).
#[test]
fn created_fifo_stats_as_fifo() {
    let dir = Scratch::new();
    let script = "builtin mkfifoat --dirfd %CWD --mode 0600 ch %>%ch; builtin statx %ch";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    let out = stdout(&out);
    assert!(
        out.contains("kind=fifo ") && out.contains("mode=600 "),
        "script={script} stdout={out}"
    );
}

/// The message-passing round trip: one read-write handle carries a line
/// written to it back to `read -u`, with no temp file and no second end.
#[test]
fn fifo_round_trip_delivers_the_line() {
    let dir = Scratch::new();
    let script = "builtin mkfifoat --mode 0600 ch %>%ch; \
                  echo \"ping\" >%ch; \
                  read -u %ch L; \
                  echo \"got $L\"";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "got ping\n", "script={script}");
}

/// A second `mkfifoat` on the same path fails EEXIST (17) and the capture is
/// never committed: no incomplete-capture machinery runs for a failing child.
/// `umask 0o022` pins the mode-less second invocation's default (`0666 & ~umask
/// = 0644`) so the mode guard cannot fire before `mkfifoat` reports EEXIST.
#[test]
fn existing_path_is_eexist() {
    let dir = Scratch::new();
    let script = "umask 0o022; builtin mkfifoat --mode 0600 ch %>%ch; builtin mkfifoat ch %>%x";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(17),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        !stderr(&out).contains("incomplete capture"),
        "script={script} stderr={}",
        stderr(&out)
    );
}

/// `--help` exits 0 without creating or capturing anything.
#[test]
fn help_exits_zero() {
    let dir = Scratch::new();
    let script = "builtin mkfifoat --help";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
}

/// An unknown flag is rejected with a message and rc 1.
#[test]
fn unknown_flag_is_rejected() {
    let dir = Scratch::new();
    let script = "builtin mkfifoat --bogus ch %>%ch";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(1),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("flag"),
        "script={script} stderr={}",
        stderr(&out)
    );
}

/// A tagged capture slot (`%fifo>%ch`) binds the `fifo`-tagged handle.
#[test]
fn tagged_capture_binds_the_fifo() {
    let dir = Scratch::new();
    let script = "builtin mkfifoat --mode 0600 ch %fifo>%ch; builtin statx %ch";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("kind=fifo"),
        "script={script} stdout={}",
        stdout(&out)
    );
}

/// The reported trap, exactly as filed: `builtin mkfifoat ch %>%ch` with no
/// `--mode` used to fail `EACCES` (13) on its own `O_RDWR` reopen.
#[test]
fn mode_less_fifo_round_trip() {
    let dir = Scratch::new();
    let script = "umask 0o022; builtin mkfifoat ch %>%ch; \
                  echo \"ping\" >%ch; \
                  read -u %ch L; \
                  echo \"got $L\"";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "got ping\n", "script={script}");
}

/// The mode-less default is `mkfifo(1)`'s `0666`, masked by the pinned umask.
#[test]
fn mode_less_fifo_mode_is_644() {
    let dir = Scratch::new();
    let script = "umask 0o022; builtin mkfifoat ch %>%ch; builtin statx %ch";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    let out = stdout(&out);
    assert!(
        out.contains("kind=fifo ") && out.contains("mode=644 "),
        "script={script} stdout={out}"
    );
}

/// A mode the `O_RDWR` reopen cannot survive is rejected in parse: rc 1, a
/// message naming `--mode`, and nothing created on disk.
#[test]
fn denied_mode_reports_mode_and_leaves_nothing() {
    let dir = Scratch::new();
    let script = "builtin mkfifoat --mode 400 ch %>%ch";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(1),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("--mode"),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        !dir.join("ch").exists(),
        "script={script} littered the disk"
    );
}

/// The guard applies the umask the kernel would: `umask 0777` masks the `0666`
/// default down to `000` and is refused, naming `--mode`.
#[test]
fn umask_masked_default_is_refused() {
    let dir = Scratch::new();
    let script = "umask 0o777; builtin mkfifoat ch %>%ch";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(1),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("--mode"),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        !dir.join("ch").exists(),
        "script={script} littered the disk"
    );
}
