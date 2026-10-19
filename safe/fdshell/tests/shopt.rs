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

impl AsRef<std::path::Path> for Scratch {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

/// A pid + counter scratch dir `fdshell-shopt-<tag>-<pid>-<counter>`,
/// removed when the test's guard drops.
fn scratch(tag: &str) -> Scratch {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell-shopt-{tag}-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// A pid + counter scratch dir `fdshell-shopt-<tag>-<pid>-<counter>`;
/// returns the path of a `.txt` file in it plus the guard — bind both.
fn temp_path(tag: &str) -> (String, Scratch) {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell-shopt-{tag}-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    (
        dir.join(format!("{tag}.txt")).to_str().unwrap().to_string(),
        Scratch(dir),
    )
}

#[test]
fn noclobber_blocks_overwrite() {
    let (path, _scratch) = temp_path("noclobber");
    std::fs::write(&path, b"keep\n").unwrap();
    let (out, err, code) = run(&format!("set -o noclobber; echo hi >{path}"));
    assert_ne!(code, 0);
    assert!(err.contains("noclobber"), "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
}

// POSIX #2.2 `>|`: the clobber operator opens the same file `>` does, but
// bypasses `noclobber`. bash 5.3.9: `set -o noclobber; echo NEW >|f` (f exists)
// exits 0 with `f` = `NEW`; the `>` control exits 1 and leaves `f` unchanged.
#[test]
fn clobber_bypasses_noclobber_on_an_existing_file() {
    let (path, _scratch) = temp_path("clobber");
    std::fs::write(&path, b"keep\n").unwrap();
    let (out, err, code) = run(&format!("set -o noclobber; echo NEW >|{path}"));
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    assert_eq!(content, "NEW\n");

    // The `>` control: noclobber still blocks, so the bypass is the operator.
    std::fs::write(&path, b"keep\n").unwrap();
    let (_out2, err2, code2) = run(&format!("set -o noclobber; echo NEW >{path}"));
    let content2 = std::fs::read_to_string(&path).unwrap();
    assert_eq!(code2, 1, "stderr={err2:?}");
    assert!(err2.contains("noclobber"), "stderr={err2:?}");
    assert_eq!(content2, "keep\n");
}

// `>|` also creates a file that does not exist (bash 5.3.9: rc 0, `hi\n`).
#[test]
fn clobber_creates_a_new_file_under_noclobber() {
    let (path, _scratch) = temp_path("clobber_new");
    let (out, err, code) = run(&format!("set -o noclobber; echo hi >|{path}"));
    let content = std::fs::read_to_string(&path);
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(content.unwrap(), "hi\n");
    assert!(out.is_empty(), "stdout={out:?}");
}

// `2>|` bypasses noclobber on stderr. bash 5.3.9: `exec 2>|f` with `f` existing
// succeeds and `echo e >&2` lands in `f`.
#[test]
fn clobber_bypasses_noclobber_on_stderr() {
    let (path, _scratch) = temp_path("clobber_stderr");
    std::fs::write(&path, b"keep\n").unwrap();
    let (out, err, code) = run(&format!(
        "set -o noclobber; exec 2>|{path}; echo e >&2; echo out; cat {path}"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "out\ne\n", "stdout={out:?}");
}

// `exec >|file` is permanent, and noclobber does not block it (bash 5.3.9: rc 0,
// later output lands in the file). The `exec >file` control fails under
// noclobber and leaves the file unchanged.
#[test]
fn exec_clobber_is_permanent_and_bypasses_noclobber() {
    let (path, _scratch) = temp_path("clobber_exec");
    std::fs::write(&path, b"keep\n").unwrap();
    let (out, err, code) = run(&format!("set -o noclobber; exec >|{path}; echo hi"));
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    assert_eq!(content, "hi\n");

    std::fs::write(&path, b"keep\n").unwrap();
    let (_out2, err2, code2) = run(&format!("set -o noclobber; exec >{path}; echo hi"));
    let content2 = std::fs::read_to_string(&path).unwrap();
    assert_eq!(code2, 1, "stderr={err2:?}");
    assert_eq!(content2, "keep\n");
}

// The bypass is the direction, not the option: a `>|` redirect never mutates
// the `noclobber` state, so the following `>` is still blocked and `set -o`
// still reports the option on (bash 5.3.9: `set -o` prints `noclobber on`).
#[test]
fn clobber_does_not_change_the_noclobber_option() {
    let (path, _scratch) = temp_path("clobber_state");
    std::fs::write(&path, b"keep\n").unwrap();
    let (out, err, code) = run(&format!(
        "set -o noclobber; echo NEW >|{path}; set -o; echo hi >{path}"
    ));
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.contains("noclobber on"), "stdout={out:?}");
    // `>|` truncated the file, then the `>` control blocked on the (now
    // existing) file, so the command aborts with the file left at `NEW`.
    assert_eq!(content, "NEW\n");
}

#[test]
fn noclobber_allows_new_files() {
    let (path, _scratch) = temp_path("newfile");
    let _ = std::fs::remove_file(&path);
    let (_out, err, code) = run(&format!("set -o noclobber; echo hi >{path}"));
    let content = std::fs::read_to_string(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(content.unwrap(), "hi\n");
}

#[test]
fn shopt_u_reenables_overwrite_in_same_shell() {
    let (path, _scratch) = temp_path("toggle");
    std::fs::write(&path, b"old\n").unwrap();
    let (_out, err, code) = run(&format!(
        "shopt -s noclobber; shopt -u noclobber; echo new >{path}"
    ));
    let content = std::fs::read_to_string(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(content.unwrap(), "new\n");
}

#[test]
fn shopt_query_exit_codes() {
    let (_out, _err, code) = run("shopt -s noclobber; shopt -q noclobber");
    assert_eq!(code, 0);
    let (_out, _err, code) = run("shopt -q noclobber");
    assert_eq!(code, 1);
}

#[test]
fn expand_aliases_on_by_default() {
    let (_out, _err, code) = run("shopt -q expand_aliases");
    assert_eq!(code, 0);
    let (_out, _err, code) = run("shopt -u expand_aliases; shopt -q expand_aliases");
    assert_eq!(code, 1);
}

#[test]
fn set_dash_o_toggles_option() {
    let (_out, _err, code) = run("set -o noclobber; shopt -q noclobber");
    assert_eq!(code, 0);
    let (_out, _err, code) = run("set +o noclobber; shopt -q noclobber");
    assert_eq!(code, 1);
}

#[test]
fn unknown_option_fails_actionably() {
    let (_out, err, code) = run("set -o bogus_option");
    assert_ne!(code, 0);
    assert!(err.contains("bogus_option"), "stderr={err:?}");
    let (_out, err, code) = run("shopt -s bogus_option");
    assert_ne!(code, 0);
    assert!(err.contains("bogus_option"), "stderr={err:?}");
}

#[test]
fn shopt_lists_options() {
    let (out, err, code) = run("shopt");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.contains("noclobber off"), "stdout={out:?}");
    assert!(out.contains("expand_aliases on"), "stdout={out:?}");
}

#[test]
fn noclobber_readonly_dir_is_open_error() {
    let dir = scratch("ro");
    let ro = std::os::unix::fs::PermissionsExt::from_mode(0o555);
    std::fs::set_permissions(&dir, ro).unwrap();
    let target_path = dir.join("newfile");
    let target = target_path.to_str().unwrap();
    let (_out, err, code) = run(&format!("set -o noclobber; echo hi >{target}"));
    std::fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    assert_ne!(code, 0);
    assert!(!err.contains("noclobber"), "stderr={err:?}");
    assert!(err.contains("open"), "stderr={err:?}");
}

#[test]
fn shopt_unknown_flag_fails() {
    let (_out, err, code) = run("shopt -z noclobber");
    assert_ne!(code, 0);
    assert!(err.contains("-z"), "stderr={err:?}");
}

#[test]
fn set_dash_o_lists_options() {
    let (out, err, code) = run("set -o noclobber; set -o");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.contains("noclobber on"), "stdout={out:?}");
    assert!(out.contains("expand_aliases on"), "stdout={out:?}");
}

#[test]
fn dollar_dash_expands_active_option_flags() {
    let (out, _err, code) = run("builtin echo [$-]");
    assert_eq!(code, 0);
    assert_eq!(out, "[]\n");
    let (out, _err, code) = run("set -o noclobber; builtin echo [$-]");
    assert_eq!(code, 0);
    assert_eq!(out, "[C]\n");
    let (out, _err, code) = run("set -o noclobber; set +o noclobber; builtin echo [$-]");
    assert_eq!(code, 0);
    assert_eq!(out, "[]\n");
}
