#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn tmpdir() -> std::path::PathBuf {
    let pid = std::process::id();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let mut p = std::env::temp_dir();
    p.push(format!("fdshell_busybox_{}_{}", pid, id));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn run_c(cmd: &str, dir: &std::path::Path) -> std::process::Output {
    Command::new(BIN)
        .args(["-c", cmd])
        .current_dir(dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap()
}

/// Open the running shell binary as an fd, then re-exec it under `name`.
fn busybox(name: &str, args: &str, dir: &std::path::Path) -> std::process::Output {
    let open = "builtin openat2 --flags O_RDONLY /proc/self/exe %>%exe";
    let mut exec = format!("builtin exec_fd %exe {name}");
    if !args.is_empty() {
        exec.push(' ');
        exec.push_str(args);
    }
    run_c(&format!("{open}; {exec}"), dir)
}

#[test]
fn busybox_echo() {
    let dir = tmpdir();
    let out = busybox("echo", "hello world", &dir);
    assert!(
        out.status.success(),
        "echo: exit={:?} stderr={}",
        out.status.code(),
        str::from_utf8(&out.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&out.stdout).unwrap().trim(), "hello world");
}

#[test]
fn busybox_pwd() {
    let dir = tmpdir();
    let out = busybox("pwd", "", &dir);
    assert!(out.status.success());
    assert_eq!(
        str::from_utf8(&out.stdout).unwrap().trim(),
        dir.to_str().unwrap()
    );
}

#[test]
fn busybox_true_exit_zero() {
    let dir = tmpdir();
    let out = busybox("true", "", &dir);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn busybox_false_exit_one() {
    let dir = tmpdir();
    let out = busybox("false", "", &dir);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn busybox_help_lists_builtins() {
    let dir = tmpdir();
    let out = busybox("help", "", &dir);
    assert!(out.status.success());
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(stdout.contains("echo"));
    assert!(stdout.contains("openat2"));
}

#[test]
fn busybox_renameat2() {
    let dir = tmpdir();
    std::fs::write(dir.join("src"), b"data").unwrap();
    let out = busybox("renameat2", "src dst", &dir);
    assert!(
        out.status.success(),
        "renameat2: exit={:?} stderr={}",
        out.status.code(),
        str::from_utf8(&out.stderr).unwrap()
    );
    assert!(dir.join("dst").exists());
    assert!(!dir.join("src").exists());
}

#[test]
fn busybox_path_basename_echo() {
    let dir = tmpdir();
    let link = dir.join("echo");
    std::os::unix::fs::symlink(BIN, &link).unwrap();
    let out = Command::new(&link)
        .args(["via-path"])
        .current_dir(&dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(str::from_utf8(&out.stdout).unwrap().trim(), "via-path");
}

#[test]
fn busybox_nonbuiltin_is_shell() {
    let dir = tmpdir();
    let link = dir.join("mytool");
    std::os::unix::fs::symlink(BIN, &link).unwrap();
    let out = Command::new(&link)
        .args(["-c", "echo shellran"])
        .current_dir(&dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(str::from_utf8(&out.stdout).unwrap().trim(), "shellran");
}
