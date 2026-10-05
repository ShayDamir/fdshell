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

/// Write an executable `#!/bin/sh` script named `name` in a fresh temp dir.
fn make_external(tag: &str, name: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("command_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join(name);
    std::fs::write(&exe, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    exe
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
    let exe = make_external("ext", "cmd_ext_xyz", "#!/bin/sh\necho EXTERNAL\n");
    let dir = exe.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(&path_with(dir), "command cmd_ext_xyz");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "EXTERNAL\n");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn command_runs_external_with_args() {
    let exe = make_external(
        "extargs",
        "cmd_ext_xyz",
        "#!/bin/sh\necho EXTERNAL \"$@\"\n",
    );
    let dir = exe.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(&path_with(dir), "command cmd_ext_xyz a b");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "EXTERNAL a b\n");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn command_prefers_builtin_over_path_shadow() {
    // `command` implies builtin-first: a PATH shadow of a builtin name is
    // bypassed (unlike a bare name with `builtin_first` off).
    let exe = make_external("shadow", "echo", "#!/bin/sh\necho SHADOWED \"$@\"\n");
    let dir = exe.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(&path_with(dir), "command echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
    let _ = std::fs::remove_dir_all(dir);
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
