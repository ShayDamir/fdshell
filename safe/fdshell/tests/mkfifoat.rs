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
#[test]
fn existing_path_is_eexist() {
    let dir = Scratch::new();
    let script = "builtin mkfifoat --mode 0600 ch %>%ch; builtin mkfifoat ch %>%x";
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
