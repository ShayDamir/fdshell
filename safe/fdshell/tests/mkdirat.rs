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
            std::env::temp_dir().join(format!("fdshell-mkdirat-{}-{}", std::process::id(), c));
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

// Every mode assertion in this file is pinned by setting `umask 0o022` as the
// first command: the shell applies it with `umask(2)` and the forked builtin
// children inherit it, so `mode & ~umask` is a known value (LESSONS).

/// The reported trap: a mode-less `mkdirat` used to create a `000` directory
/// whose reopen failed `EACCES` (13).
#[test]
fn mode_less_dir_is_a_dir_with_755() {
    let dir = Scratch::new();
    let script = "umask 0o022; builtin mkdirat d %>%d; builtin statx %d";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    let out = stdout(&out);
    assert!(
        out.contains("kind=dir ") && out.contains("mode=755 "),
        "script={script} stdout={out}"
    );
}

/// The default must carry the owner `x` bit: a `0644` directory cannot have
/// files created in it by path, which is why `mkdirat` defaults to `0777`.
#[test]
fn mode_less_dir_accepts_new_files() {
    let dir = Scratch::new();
    let script = "umask 0o022; builtin mkdirat d %>%d; \
                  builtin openat2 --dirfd %d --flags O_CREAT --flags O_EXCL --flags O_RDWR --mode 0600 f %>%f; \
                  echo ok";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "ok\n", "script={script}");
}

/// A mode the builtin's own reopen cannot survive is rejected in parse: rc 1, a
/// message naming `--mode`, and nothing created on disk.
#[test]
fn denied_mode_reports_mode_and_leaves_nothing() {
    let dir = Scratch::new();
    let script = "builtin mkdirat --mode 400 d %>%d";
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
    assert!(!dir.join("d").exists(), "script={script} littered the disk");
}

/// The guard applies the umask the kernel would: `umask 0777` masks the `0777`
/// default down to `000` and is refused, naming `--mode`.
#[test]
fn umask_masked_default_is_refused() {
    let dir = Scratch::new();
    let script = "umask 0o777; builtin mkdirat d %>%d";
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
    assert!(!dir.join("d").exists(), "script={script} littered the disk");
}

/// An explicit mode overrides the default, and `--help` stays rc 0.
#[test]
fn explicit_mode_overrides_the_default() {
    let dir = Scratch::new();
    let script = "umask 0o022; builtin mkdirat --mode 0700 d %>%d; builtin statx %d";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("mode=700 "),
        "script={script} stdout={}",
        stdout(&out)
    );
}
