#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// A scratch dir with a `src` file (the copy source) and an empty `dst` file;
/// both are opened via `openat2` and captured into fd variables, so the test
/// harness can read `dst` off disk after the child exits.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str, src: &[u8]) -> Self {
        // Per-test tag keeps this dir unique under nextest's parallel threads,
        // which share one pid (LESSONS.md: "Tests in one binary share the process").
        let dir = std::env::temp_dir().join(format!("fdshell-cfra-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("src"), src).unwrap();
        std::fs::write(dir.join("dst"), b"").unwrap();
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

fn read_dst(dir: &std::path::Path) -> Vec<u8> {
    std::fs::read(dir.join("dst")).unwrap()
}

const SRC: &[u8] = b"hello world";

fn copy_script(count: Option<&str>) -> String {
    match count {
        Some(n) => format!(
            "builtin openat2 --flags O_RDONLY src %>%in; \
             builtin openat2 --flags O_WRONLY dst %>%out; \
             builtin copy_file_range %in %out {n}"
        ),
        None => String::from(
            "builtin openat2 --flags O_RDONLY src %>%in; \
             builtin openat2 --flags O_WRONLY dst %>%out; \
             builtin copy_file_range %in %out",
        ),
    }
}

/// With an explicit `COUNT` equal to the source size, every byte is copied.
#[test]
fn copies_all_with_count() {
    let dir = Scratch::new("all_count", SRC);
    let out = run(&dir, &copy_script(Some("11")));
    assert_eq!(stdout(&out), "11\n", "stderr={}", stderr(&out));
    assert_eq!(read_dst(&dir), SRC);
}

/// Without a `COUNT`, all of the source is copied.
#[test]
fn copies_all_without_count() {
    let dir = Scratch::new("all_no_count", SRC);
    let out = run(&dir, &copy_script(None));
    assert_eq!(stdout(&out), "11\n", "stderr={}", stderr(&out));
    assert_eq!(read_dst(&dir), SRC);
}

/// A `COUNT` smaller than the source copies only that many bytes.
#[test]
fn copies_partial() {
    let dir = Scratch::new("partial", SRC);
    let out = run(&dir, &copy_script(Some("5")));
    assert_eq!(stdout(&out), "5\n", "stderr={}", stderr(&out));
    assert_eq!(read_dst(&dir), b"hello");
}

/// A `COUNT` larger than the available bytes fails with a message and rc 1,
/// leaving the destination unchanged.
#[test]
fn count_exceeds_available_errors() {
    let dir = Scratch::new("exceeds", SRC);
    let out = run(&dir, &copy_script(Some("100")));
    assert_eq!(out.status.code(), Some(1), "stderr={}", stderr(&out));
    assert!(stderr(&out).contains("count"), "stderr={}", stderr(&out));
    assert_eq!(read_dst(&dir), b"");
}
