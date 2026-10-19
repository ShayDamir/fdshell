#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str, stdin: &str) -> (String, String, i32) {
    let mut child = Command::new(BIN)
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). The pid-named temp files live inside this dir now. Removal is
/// `.ok()`-tolerant: teardown never panics, and it runs on a failing test's
/// unwind, which is exactly the test that used to leak.
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

/// A pid + counter scratch dir `fdshell-fdpath-<tag>-<pid>-<counter>`;
/// returns the path of a `.txt` file in it plus the guard — bind both.
fn temp_path(tag: &str) -> (String, Scratch) {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell-fdpath-{tag}-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    (
        dir.join(format!("{tag}.txt")).to_str().unwrap().to_string(),
        Scratch(dir),
    )
}

#[test]
fn fd_path_redirect_reads_pipe_stdin() {
    let (out, err, code) = run("cat </dev/fd/0", "hi\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn fd_path_redirect_reads_file_fd() {
    let (path, _scratch) = temp_path("file");
    std::fs::write(&path, b"abc\n").unwrap();
    let (out, err, code) = run(&format!("exec 3<{path}; cat </dev/fd/3; exec 3>&-"), "");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "abc\n");
}

#[test]
fn fd_path_redirect_writes_to_fd() {
    let (path, _scratch) = temp_path("write");
    let (out, err, code) = run(
        &format!("exec 3>{path}; echo hi >/dev/fd/3; exec 3>&-; cat {path}"),
        "",
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn proc_self_fd_redirect_reads_pipe_stdin() {
    let (out, err, code) = run("cat </proc/self/fd/0", "yo\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "yo\n");
}
