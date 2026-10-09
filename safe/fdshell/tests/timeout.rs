#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> std::process::Output {
    Command::new(BIN).args(["-c", script]).output().unwrap()
}

/// A scratch `PATH` dir of executable files, so `timeout`'s folded target word is
/// looked up as a real file (bash runs `timeout 1 a\*` for a file named `a*`).
fn scratch_bin(names: &[&str]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fdshell-esc-timeout-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in names {
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\necho ran\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

fn run_with_path(bin_dir: &std::path::Path, script: &str) -> (String, String, i32) {
    let path = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap());
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

/// `timeout 1 sleep 5` must time out: the child is killed and the command
/// exits 124 (matching coreutils `timeout`).
#[test]
fn timeout_times_out() {
    let out = run("timeout 1 sleep 5; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=124"),
        "expected exit=124, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `timeout 5 true` must succeed: the child finishes in time and the command
/// exits 0.
#[test]
fn timeout_success() {
    let out = run("timeout 5 true; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=0"),
        "expected exit=0, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// A child that ignores SIGTERM must still be killed: after the grace period
/// the shell force-SIGKILLs it and the command exits 124.
#[test]
fn timeout_force_kills_sigterm_ignoring_child() {
    // `sh` traps (ignores) SIGTERM, then execs `sleep`, which inherits the
    // ignored disposition. The deadline SIGTERM therefore has no effect and
    // only the grace-period SIGKILL can end the child.
    let out = run("timeout 1 sh -c \"trap \\\"\\\" TERM; exec sleep 30\"; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=124"),
        "expected exit=124, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `timeout 5 builtin false` must return the child's exit code (1), not 124.
#[test]
fn timeout_returns_child_exit_code() {
    let out = run("timeout 5 builtin false; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=1"),
        "expected exit=1, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `timeout` with no seconds is a clean error (exit 1).
#[test]
fn timeout_missing_seconds_errors() {
    let out = run("timeout");
    assert_eq!(out.status.code(), Some(1));
    let err = str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("timeout"), "stderr={err}");
}

/// `timeout 5` with no command is a clean error (exit 1).
#[test]
fn timeout_missing_command_errors() {
    let out = run("timeout 5");
    assert_eq!(out.status.code(), Some(1));
    let err = str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("timeout"), "stderr={err}");
}

/// `timeout abc true` with a bad seconds value is a clean error (exit 1).
#[test]
fn timeout_bad_seconds_errors() {
    let out = run("timeout abc true");
    assert_eq!(out.status.code(), Some(1));
    let err = str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("seconds"), "stderr={err}");
}

/// POSIX #4.1: `timeout`'s target word is folded before the lookup, as bash folds
/// word 0 at tokenization — `timeout 1 e\cho hi` runs `echo` and `timeout 1 a\*`
/// runs the file literally named `a*`. The seconds argument and the trailing args
/// (which go through substitution) are unaffected.
#[test]
fn timeout_folds_the_target_command_word() {
    let bin = scratch_bin(&["a*"]);
    let (out, err, code) = run_with_path(&bin, "timeout 1 e\\cho hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
    let (out, err, code) = run_with_path(&bin, "timeout 1 a\\*");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "ran\n");
    // The target's own args are substituted, so their pairs fold as usual.
    let (out, err, code) = run_with_path(&bin, "timeout 1 e\\cho a\\* b");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a* b\n");
    // A quoted target word keeps its pair, so the literal name is looked up.
    let (_out, err, code) = run_with_path(&bin, "timeout 1 \"e\\cho\" hi");
    assert_eq!(code, 1, "the quoted pair is not folded, stderr={err:?}");
    assert!(err.contains("e\\cho"), "stderr={err:?}");
    let _ = std::fs::remove_dir_all(&bin);
}
