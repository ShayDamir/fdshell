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

#[test]
fn dup_redirect_sends_stderr_to_stdout() {
    let (out, err, code) = run("nonexistent_xyz_abc 2>&1");
    assert_ne!(code, 0);
    assert!(out.contains("not found"), "stdout={out:?}");
    assert!(!err.contains("not found"), "stderr={err:?}");
}

#[test]
fn dup_redirect_stdout_to_fd() {
    let (out, err, code) = run("echo err >&2");
    assert_eq!(code, 0);
    assert!(!out.contains("err"), "stdout={out:?}");
    assert!(err.contains("err"), "stderr={err:?}");
}

#[test]
fn exec_fd_dup_then_error_routes_through_fd() {
    let (out, _err, code) = run("exec 5>&1; nonexistent_xyz_abc 2>&5");
    assert_ne!(code, 0);
    assert!(out.contains("not found"), "stdout={out:?}");
}

#[test]
fn close_redirect_closes_fd_in_shell() {
    let (_out, err, code) = run("exec 3>&1; exec 3>&-; exec 4>&3");
    assert_ne!(code, 0);
    assert!(err.contains("fd 3 is not open"), "stderr={err:?}");
}

/// A dup onto the first free fd (here 4) must stay open for later use. A
/// source dup landing on the target makes `dup2` a no-op whose drop closes
/// the redirected fd.
#[test]
fn exec_dup_onto_first_free_fd_stays_open() {
    let (out, err, code) = run("exec 4>&1; echo via-4 >&4; exec 4>&-; echo done");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "via-4\ndone\n", "stdout={out:?}");
}

/// A path redirect whose target equals the first free fd must not end up
/// closed; data written through the fd must reach the file.
#[test]
fn exec_path_redirect_onto_first_free_fd_survives() {
    let (path, _scratch) = temp_path("first_free");
    let (out, err, code) = run(&format!(
        "exec 5>{path}; echo via-5 >&5; exec 5>&-; cat {path}"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "via-5\n", "stdout={out:?}");
}

/// Two path redirects whose targets interleave with the pre-opened fd
/// numbers must both keep their targets: one redirect's dup2 must not close
/// the other's target via a pre-opened descriptor that shared its number.
#[test]
fn exec_multi_redirect_targets_keep_their_files() {
    let (a, _sa) = temp_path("multi_a");
    let (b, _sb) = temp_path("multi_b");
    let (out, err, code) = run(&format!(
        "exec 5>{a} 4>{b}; echo to-fd4 >&4; echo to-fd5 >&5; exec 4>&-; exec 5>&-; cat {b} {a}"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "to-fd4\nto-fd5\n", "stdout={out:?}");
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

/// A pid + counter scratch dir `fdshell-fddup-<tag>-<pid>-<counter>`;
/// returns the path of a `.txt` file in it plus the guard — bind both.
fn temp_path(tag: &str) -> (String, Scratch) {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell-fddup-{tag}-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    (
        dir.join(format!("{tag}.txt")).to_str().unwrap().to_string(),
        Scratch(dir),
    )
}

#[test]
fn exec_fd_dup_writes_via_fd() {
    let (path, _scratch) = temp_path("write");
    let (out, err, code) = run(&format!(
        "exec 3>{path}; echo hi >&3; exec 3>&-; cat {path}"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn exec_fd_dup_reads_via_fd() {
    let (path, _scratch) = temp_path("read");
    std::fs::write(&path, b"hi\n").unwrap();
    let (out, err, code) = run(&format!("exec 3<{path}; cat <&3; exec 3>&-"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn out_of_range_path_redirect_is_actionable() {
    let (path, _scratch) = temp_path("range");
    let (_out, err, code) = run(&format!("true 2147483647>{path}"));
    assert_ne!(code, 0);
    assert!(err.contains("out of range"), "stderr={err:?}");
}

#[test]
fn dup_of_closed_fd_is_actionable() {
    let (_out, err, code) = run("cat 2>&99");
    assert_ne!(code, 0);
    assert!(err.contains("fd 99 is not open"), "stderr={err:?}");
}

/// `>&%var` copies the fd the variable refers to onto the target: echo's
/// stdout becomes the file, so the file receives the echo.
#[test]
fn dup_var_redirect_writes_through_fd_var() {
    let (path, _scratch) = temp_path("dup_var_write");
    std::fs::write(&path, b"").unwrap();
    let (out, err, code) = run(&format!(
        "builtin openat2 --flags O_RDWR {path} %>%f; echo hi >&%f; cat {path}"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n", "stdout={out:?}");
}

/// `<&%var` copies the fd the variable refers to onto stdin: cat reads the
/// file through the duplicated descriptor.
#[test]
fn dup_var_redirect_reads_through_fd_var() {
    let (path, _scratch) = temp_path("dup_var_read");
    std::fs::write(&path, b"hi\n").unwrap();
    let (out, err, code) = run(&format!(
        "builtin openat2 --flags O_RDONLY {path} %>%f; cat <&%f"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n", "stdout={out:?}");
}

#[test]
fn dup_var_redirect_missing_var_is_actionable() {
    let (_out, err, code) = run("cat 2>&%nope");
    assert_ne!(code, 0);
    assert!(err.contains("fd variable"), "stderr={err:?}");
}
