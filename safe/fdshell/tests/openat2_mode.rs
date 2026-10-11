#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Per-test scratch dir, removed on drop; pid + atomic counter naming, like
/// every other e2e harness here (LESSONS).
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let c = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("fdshell-openat2-mode-{}-{}", std::process::id(), c));
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

/// `openat2 --flags O_CREAT` with no `--mode` creates the file with the POSIX
/// `open(2)` default (`0666`), which the pinned `umask 0o022` masks to `0644`
/// — used to be a `0000` file, unreadable and unwritable by its own owner.
#[test]
fn create_mode_default_is_masked_by_the_umask() {
    let dir = Scratch::new();
    let script = "umask 0o022; \
                  builtin openat2 --dirfd %CWD --flags O_CREAT --flags O_EXCL --flags O_RDWR f %>%f; \
                  echo hi >%f; \
                  builtin openat2 --dirfd %CWD --flags O_RDONLY f %>%g; \
                  read -u %g L; \
                  echo \"got $L\"; \
                  builtin statx %f";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    let out = stdout(&out);
    assert!(
        out.contains("got hi\n") && out.contains("kind=file ") && out.contains("mode=644 "),
        "script={script} stdout={out}"
    );
}

/// An explicit `--mode 0` is the user's choice, not the default: it stays 0.
/// The open is `O_RDONLY`, which needs no permission bits.
#[test]
fn explicit_mode_zero_is_kept() {
    let dir = Scratch::new();
    let script = "umask 0o022; \
                  builtin openat2 --dirfd %CWD --flags O_CREAT --flags O_EXCL --mode 0 g %>%g; \
                  builtin statx %g";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("mode=0 "),
        "script={script} stdout={}",
        stdout(&out)
    );
}
