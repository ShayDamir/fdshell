#![allow(clippy::unwrap_used)]

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

fn run_with_path(path: &str, script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .env("PATH", path)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

fn path_with(dir: &str) -> String {
    format!("{dir}:{}", std::env::var("PATH").unwrap_or_default())
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). Removal is `.ok()`-tolerant: teardown never panics, and it
/// runs on a failing test's unwind, which is exactly the test that used to
/// leak.
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

/// Write an executable `#!/bin/sh` script named `name` in a fresh temp dir.
fn make_external(tag: &str, name: &str, body: &str) -> (std::path::PathBuf, Scratch) {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("command_{tag}_{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join(name);
    std::fs::write(&exe, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    (exe, Scratch(dir))
}

#[test]
fn command_runs_builtin() {
    let (out, err, code) = run("command echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn command_bypasses_function_lookup() {
    let (out, _err, code) = run("echo() { builtin echo shadowed; }; command echo hi; echo hi");
    assert_eq!(code, 0);
    assert_eq!(out, "hi\nshadowed\n");
}

#[test]
fn command_unknown_name_falls_through_to_path() {
    // A `command`-qualified name that is not a builtin falls through to the
    // PATH search and fails like a bare missing command (resolve error, exit 1).
    let (_out, err, code) = run("command definitely_not_a_cmd_xyz");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("definitely_not_a_cmd_xyz"), "stderr={err:?}");
    assert!(err.contains("not found"), "stderr={err:?}");
}

#[test]
fn command_runs_external() {
    let (exe, _scratch) = make_external("ext", "cmd_ext_xyz", "#!/bin/sh\necho EXTERNAL\n");
    let dir = exe.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(&path_with(dir), "command cmd_ext_xyz");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "EXTERNAL\n");
}

#[test]
fn command_runs_external_with_args() {
    let (exe, _scratch) = make_external(
        "extargs",
        "cmd_ext_xyz",
        "#!/bin/sh\necho EXTERNAL \"$@\"\n",
    );
    let dir = exe.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(&path_with(dir), "command cmd_ext_xyz a b");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "EXTERNAL a b\n");
}

#[test]
fn command_prefers_builtin_over_path_shadow() {
    // `command` implies builtin-first: a PATH shadow of a builtin name is
    // bypassed (unlike a bare name with `builtin_first` off).
    let (exe, _scratch) = make_external("shadow", "echo", "#!/bin/sh\necho SHADOWED \"$@\"\n");
    let dir = exe.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(&path_with(dir), "command echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn builtin_non_builtin_still_errors() {
    // `builtin NAME` stays builtin-only: a non-builtin is an error.
    let (_out, err, code) = run("builtin definitely_not_a_cmd_xyz");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("is not a shell builtin"), "stderr={err:?}");
}

#[test]
fn command_prefix_rejected_on_intercepts() {
    let (_out, err, code) = run("command cd /tmp");
    assert_ne!(code, 0);
    assert!(err.contains("prefix is not supported"), "stderr={err:?}");
}
