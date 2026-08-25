#![allow(clippy::unwrap_used)]

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn temp_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("builtin_first_{tag}_{}", std::process::id()))
}

fn make_shadow(tag: &str) -> std::path::PathBuf {
    let dir = temp_dir(tag);
    std::fs::create_dir_all(&dir).unwrap();
    let shadow = dir.join("openat2");
    std::fs::write(&shadow, "#!/bin/sh\necho EXTERNAL\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shadow, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    shadow
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

#[test]
fn without_option_bare_name_reaches_path() {
    let shadow = make_shadow("off");
    let dir = shadow.parent().unwrap().to_str().unwrap();
    let (out, _err, _code) = run_with_path(&path_with(dir), "openat2");
    assert_eq!(out, "EXTERNAL\n");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn builtin_first_resolves_bare_name_as_builtin() {
    let shadow = make_shadow("on");
    let dir = shadow.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(
        &path_with(dir),
        "set -o builtin_first; openat2 missing_file_xyz %>%z",
    );
    assert_ne!(code, 0, "stderr={err:?}");
    assert!(!out.contains("EXTERNAL"), "stdout={out:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn builtin_first_still_allows_explicit_path() {
    let shadow = make_shadow("path");
    let dir = shadow.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(
        &path_with(dir),
        &format!("set -o builtin_first; {}", shadow.display()),
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "EXTERNAL\n");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn builtin_keyword_still_works_with_option_off() {
    let (out, _err, code) = run_with_path("/usr/bin:/bin", "builtin echo kw");
    assert_eq!(code, 0);
    assert_eq!(out, "kw\n");
}
