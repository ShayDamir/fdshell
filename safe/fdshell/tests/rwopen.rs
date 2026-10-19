#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
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

/// A pid + counter scratch dir `fdshell-rwopen-<tag>-<pid>-<counter>`;
/// returns the path of a `.txt` file in it plus the guard — bind both.
fn temp_path(tag: &str) -> (String, Scratch) {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell-rwopen-{tag}-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    (
        dir.join(format!("{tag}.txt")).to_str().unwrap().to_string(),
        Scratch(dir),
    )
}

#[test]
fn rw_redirect_reads_existing_content() {
    let (path, _scratch) = temp_path("read");
    std::fs::write(&path, b"abc").unwrap();
    let (out, err, code) = run(&format!("exec 3<>{path}; cat <&3; exec 3>&-"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "abc");
}

#[test]
fn rw_redirect_keeps_content_and_shares_file_offset() {
    let (path, _scratch) = temp_path("offset");
    std::fs::write(&path, b"abc").unwrap();
    let (out, err, code) = run(&format!(
        "exec 2<>{path}; echo d >&2; cat <&2; exec 2>&-; cat {path}"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    // `echo d` overwrites "ab" with "d\n" (file is now "d\nc");
    // `cat <&2` resumes at offset 2 ("c"), then `cat` prints the whole file.
    assert_eq!(out, "cd\nc");
}

#[test]
fn rw_redirect_creates_missing_file() {
    let (path, _scratch) = temp_path("create");
    let (_out, err, code) = run(&format!("exec 3<>{path}; exec 3>&-"));
    let exists = std::path::Path::new(&path).exists();
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(exists);
}
