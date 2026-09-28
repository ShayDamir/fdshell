#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::ops::Deref;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A scratch dir with a write-only (0200) 5-byte `f`; each test gets its own
/// dir (pid + counter: tests in one binary share the pid); removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("fdshell-openat2-path-{}-{}", std::process::id(), c));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("f");
        std::fs::write(&f, b"12345").unwrap();
        let mut perms = std::fs::metadata(&f).unwrap().permissions();
        perms.set_mode(0o200);
        std::fs::set_permissions(&f, perms).unwrap();
        Self(dir)
    }
}

impl Deref for Scratch {
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

/// True when the test process can read the write-only `f` (root /
/// CAP_DAC_OVERRIDE, e.g. a nix sandbox); in that case the EACCES assertion
/// is skipped.
fn can_read(dir: &std::path::Path) -> bool {
    std::fs::File::open(dir.join("f")).is_ok()
}

/// A plain `O_RDONLY` open of the write-only file is denied, but `--path`
/// (O_PATH) opens it: the inspect-then-act entry.
#[test]
fn path_opens_where_read_open_is_denied() {
    let dir = Scratch::new();
    let read_ok = can_read(&dir);
    if read_ok {
        eprintln!("test process can read the write-only file; skipping EACCES assertion");
    }
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY f; builtin echo rc=$?",
    );
    if !read_ok {
        assert!(stdout(&out).contains("rc=13"), "stdout={:?}", stdout(&out));
    }
    let out = run(&dir, "builtin openat2 --path f %>%p; builtin echo rc=$?");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(stdout(&out), "rc=0\n", "stderr={}", stderr(&out));
}

/// `statx %fd` re-stats the O_PATH handle (AT_EMPTY_PATH): the file's
/// metadata is readable without open permission.
#[test]
fn statx_on_path_handle_reports_metadata() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin openat2 --path f %>%p; builtin statx %p; builtin echo rc=$?",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
    let out = stdout(&out);
    assert!(out.contains("kind=file size=5"), "stdout={out:?}");
    assert!(out.contains("rc=0"), "stdout={out:?}");
}

/// An O_PATH handle carries no open permission: `ftruncate` fails EBADF
/// (errno 9; probed on kernel 7.2.4) and the file is untouched.
#[test]
fn path_handle_has_no_open_permission() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin openat2 --path f %>%p; builtin ftruncate %p 3; \
         builtin echo rc=$?; builtin statx f",
    );
    let err = stderr(&out);
    let out = stdout(&out);
    assert!(out.contains("rc=9"), "stdout={out:?} stderr={err}");
    assert!(out.contains("size=5"), "stdout={out:?}");
    assert_eq!(err, "", "errno errors must be silent");
}
