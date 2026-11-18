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

/// Run with every fd in `fds` **closed from the start**: `sh` closes them before
/// `exec`-ing fdshell, so the fd table the child sees — and therefore which fd a
/// redirect's saved copy lands on — is deterministic whatever the harness
/// inherited.
///
/// LESSONS: "A pin that asserts "the parent's fd N is closed" must close it in the child's start state (task #276)"
fn run_fds_closed(fds: &[u32], script: &str) -> (String, String, i32) {
    let mut cmd = String::new();
    for fd in fds {
        cmd.push_str(&format!("exec {fd}>&-; "));
    }
    cmd.push_str(r#"exec "$0" -c "$1""#);
    let output = Command::new("sh")
        .args(["-c", &cmd, BIN, script])
        .stdin(std::process::Stdio::null())
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

/// A pid + counter scratch dir `fdshell-scope-<tag>-<pid>-<counter>`;
/// returns the path of a `.txt` file in it plus the guard — bind both.
fn temp_path(tag: &str) -> (String, Scratch) {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell-scope-{tag}-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    (
        dir.join(format!("{tag}.txt")).to_str().unwrap().to_string(),
        Scratch(dir),
    )
}

/// Read the redirect target and remove it.
fn body(path: &str) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let _ = std::fs::remove_file(path);
    text
}

fn write_file(path: &str, text: &str) {
    std::fs::write(path, text).unwrap();
}

/// POSIX #2.4: a redirection belongs to the simple command, and a function call
/// is a simple command. `f`'s stdout goes to the file, and the shell's stdout is
/// back for the next command.
#[test]
fn function_write_redirect_applies_and_is_scoped() {
    let (a, _sa) = temp_path("fn_write_a");
    let (b, _sb) = temp_path("fn_write_b");
    let (out, err, code) = run(&format!(
        "f(){{ echo in-f; }}; f > {a}; echo after > {b}; echo A=[$(cat {a})] B=[$(cat {b})]"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "A=[in-f] B=[after]\n", "stdout={out:?}");
}

/// `f < in` feeds the function body's stdin; the shell's stdin is untouched.
#[test]
fn function_stdin_redirect_applies() {
    let (in_path, _sinpath) = temp_path("fn_read_in");
    write_file(&in_path, "hello\n");
    let (out, err, code) = run(&format!(
        "f(){{ read v; echo \"[in-f $v]\"; }}; f < {in_path}; echo after"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[in-f hello]\nafter\n", "stdout={out:?}");
}

/// `f <<EOF` puts the here-doc body on the function body's stdin.
#[test]
fn function_heredoc_applies() {
    let (out, err, code) = run("f(){ cat; echo body; }; f <<EOF\nX\nEOF");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "X\nbody\n", "stdout={out:?}");
}

/// Two redirections on one call: the last to fd 1 wins, and the earlier target
/// was still opened/truncated.
#[test]
fn function_redirect_is_last_wins() {
    let (a, _sa) = temp_path("fn_last_a");
    let (b, _sb) = temp_path("fn_last_b");
    let (out, err, code) = run(&format!("f(){{ echo hi; }}; f > {a} > {b}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "");
    assert_eq!(body(&b), "hi\n");
}

/// A nested call inherits the outer scope, and restore of the outer scope puts
/// the shell's fd 1 back so the next command writes to stdout.
#[test]
fn nested_function_redirect_restores_the_outer_first() {
    let (a, _sa) = temp_path("nested_a");
    let (b, _sb) = temp_path("nested_b");
    let (out, err, code) = run(&format!(
        "f(){{ g(){{ echo g; }}; g; echo f; }}; f > {a}; echo after > {b}"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "g\nf\n");
    assert_eq!(body(&b), "after\n");
}

/// `cd` is in-process: its redirect is applied by the scope (the file is created
/// in the pre-`cd` directory), and the `cd` itself persists.
#[test]
fn cd_redirect_applies_and_is_scoped() {
    let (a, _sa) = temp_path("cd_a");
    let (out, err, code) = run(&format!("cd /tmp > {a}; pwd; echo after"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "/tmp\nafter\n", "stdout={out:?}");
}

/// `eval` runs in the shell, so its redirect reaches the evaluated command.
#[test]
fn eval_redirect_applies() {
    let (a, _sa) = temp_path("eval_a");
    let (out, err, code) = run(&format!("eval echo hi > {a}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "hi\n");
}

/// `source` runs the file in the shell, so the redirect applies to it.
#[test]
fn source_redirect_applies() {
    let (src, _ssrc) = temp_path("eval_src");
    let (a, _sa) = temp_path("source_a");
    write_file(&src, "echo sourced\n");
    let (out, err, code) = run(&format!("source {src} > {a}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "sourced\n");
}

/// `: ` applies each redirect form (the task #152 pins): a write target is
/// created empty, a read target is consumed and discarded, an error redirect
/// opens the file with nothing written to it.
#[test]
fn colon_applies_redirect() {
    let (a, _sa) = temp_path("colon_write");
    let (in_path, _sinpath) = temp_path("colon_read");
    let (e, _se) = temp_path("colon_err");
    write_file(&in_path, "hello\n");
    let (out, err, code) = run(&format!(": > {a}; : < {in_path}; : 2> {e}; echo done"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "done\n", "stdout={out:?}");
    assert_eq!(body(&a), "");
    assert_eq!(body(&e), "");
    assert_eq!(
        body(&in_path),
        "hello\n",
        "a read redirect does not consume the file"
    );
}

/// `shift` is in-process, so its redirect is applied by the scope: `a` is
/// created and empty. Measured on bash 5.3.9, `bash -c 'shift > /tmp/s1; echo
/// rc=$?'` creates the empty file and prints `rc=1` — bash fails a `shift` with
/// no arguments. fdshell runs the empty shift successfully, so rc 0 is pinned;
/// the rc divergence is accepted and documented in README.
#[test]
fn shift_applies_redirect() {
    let (a, _sa) = temp_path("shift_a");
    let (out, err, code) = run(&format!("shift > {a}"));
    assert_eq!(
        code, 0,
        "fdshell runs the empty shift at rc 0, bash at rc 1; stderr={err:?}"
    );
    assert_eq!(out, "");
    assert_eq!(body(&a), "");
}

/// `local` inside a function: the redirect applies for the command, `local`
/// itself has no output, and stdout is back for the next command.
#[test]
fn local_applies_redirect_inside_a_function() {
    let (a, _sa) = temp_path("local_a");
    let (out, err, code) = run(&format!("f(){{ local x=1 > {a}; echo after; }}; f"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "after\n", "stdout={out:?}");
    assert_eq!(body(&a), "");
}

/// `times` prints through the scope's redirected fd 1; its exact format is
/// fdshell's own, so only the fact that output reached the file is pinned.
#[test]
fn times_redirect_writes_to_the_file() {
    let (a, _sa) = temp_path("times_a");
    let (out, err, code) = run(&format!("times > {a}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert!(!body(&a).is_empty(), "times output must reach the file");
}

/// A forked external under `timeout` inherits the redirection of the command.
#[test]
fn timeout_redirect_reaches_the_child() {
    let (a, _sa) = temp_path("timeout_a");
    let (out, err, code) = run(&format!("timeout 1 echo hi > {a}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "hi\n");
}

/// `exec > file` has no command to run, so the redirect stays: the shell's fd 1
/// is the file for the rest of the script (a scoped restore does not undo it).
#[test]
fn exec_redirect_stays_permanent() {
    let (a, _sa) = temp_path("exec_permanent");
    let (out, err, code) = run(&format!("exec > {a}; echo after"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "after\n");
}

/// `exec > file` inside a function body is scoped to the call: after the call
/// the shell's fd 1 is back, so the next command's redirect writes to its file.
#[test]
fn exec_inside_function_restores_at_call_end() {
    let (a, _sa) = temp_path("exec_in_fn_a");
    let (b, _sb) = temp_path("exec_in_fn_b");
    let (out, err, code) = run(&format!("f(){{ exec > {a}; }}; f; echo after > {b}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "");
    assert_eq!(body(&b), "after\n");
}

/// A `N>&-` target is closed for the duration of the call and reopened at the
/// restore: fd 3 is closed inside `f` and usable again after it.
#[test]
fn closed_fd_redirect_is_restored_after_the_call() {
    let (in_path, _sinpath) = temp_path("closed_in");
    let (a, _sa) = temp_path("closed_a");
    write_file(&in_path, "hello\n");
    let (out, err, code) = run(&format!(
        "exec 3< {in_path}; f(){{ echo x; }}; f > {a} 3>&-; cat <&3; echo after"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hello\nafter\n", "stdout={out:?}");
    assert_eq!(body(&a), "x\n");
}

/// The saved copy sits **above the range a script can name** (`MIN_SHELL_FD`, as
/// bash keeps its own bookkeeping), so `f(){ echo ok; exec 6>&-; }; f >q` no longer
/// closes the shell's copy: fd 6 is a never-opened fd, so the failure is the
/// `exec`'s **own** close — the report gains the `failed to close redirection target
/// fd 6` middle frame — the body's command fails, the function returns 1, the shell
/// **continues**, the restore succeeds and the next command's stdout works. bash
/// 5.3.9 runs both rows silently at rc 0; the report — the close of an
/// already-closed fd — is the rule **#613** owns. fd 6 is closed in the child's start
/// state so the fd table is deterministic (LESSONS, task #276); the unit twin that
/// measures the copy fd is
/// `run/parent/tests.rs::body_that_closes_the_saved_copy_fails_the_command`.
#[test]
fn body_that_closes_the_saved_copy_fails_the_call() {
    let (a, _sa) = temp_path("lost_save_a");
    let (out, err, code) = run_fds_closed(
        &[6],
        &format!("f(){{ echo ok; exec 6>&-; }}; f > {a}; echo after"),
    );
    assert_eq!(
        code, 0,
        "the shell continues after the failed command: stderr={err:?}"
    );
    assert_eq!(out, "after\n", "stdout is back for the next command");
    assert_eq!(body(&a), "ok\n", "the body ran under the redirection");
    assert!(
        err.contains("failed to close redirection target fd 6"),
        "the report names the fd the exec could not close: stderr={err:?}"
    );

    let (b, _sb) = temp_path("lost_save_b");
    let (out, err, code) = run_fds_closed(&[6], &format!("f(){{ echo ok; exec 6>&-; }}; f > {b}"));
    assert_eq!(
        code, 1,
        "the failed close fails the command; stderr={err:?}"
    );
    assert_eq!(out, "");
    assert!(
        err.contains("redirection failed")
            && err.contains("failed to close redirection target fd 6"),
        "the close arm, not the restore: stderr={err:?}"
    );
    assert_eq!(body(&b), "ok\n", "the body ran under the redirection");
}

/// The floor seen from a script: with fds 3-9 closed in the child's start state,
/// `f > a` spends fd 10 on the target's fresh open, fd 11 on the resolved source
/// clone and fd **12** on the saved copy of stdout (strace: `fcntl(4,
/// F_DUPFD_CLOEXEC, 10) = 10`, `fcntl(10, F_DUPFD_CLOEXEC, 10) = 11`, `fcntl(1,
/// F_DUPFD_CLOEXEC, 10) = 12`). Closing fd 6 — below the floor — cannot reach any of
/// them (`body_that_closes_the_saved_copy_fails_the_call`); closing fd 12 closes the
/// copy the parent's restore needs, so the close itself succeeds, the restore fails
/// with the two-context report (**no** `failed to close redirection target fd N`
/// middle frame) and the run ends at rc 1. bash 5.3.9 keeps its own bookkeeping
/// above the script-visible range too: rc 0, no message, and the `echo`'s `rc=0`
/// lands on stdout.
#[test]
fn body_that_closes_the_saved_copy_above_the_floor_fails_the_restore() {
    let (a, _sa) = temp_path("floor_save_a");
    let (out, err, code) = run_fds_closed(
        &[3, 4, 5, 6, 7, 8, 9],
        &format!("f(){{ exec 12>&-; }}; f > {a}; echo rc=$?"),
    );
    assert_eq!(code, 1, "the restore failure ends the run: stderr={err:?}");
    assert_eq!(out, "", "the `echo` never ran");
    assert_eq!(body(&a), "", "the body wrote nothing under the redirection");
    assert!(
        err.contains("redirection failed") && err.contains("Bad file descriptor"),
        "the restore's report: stderr={err:?}"
    );
    assert!(
        !err.contains("failed to close redirection target"),
        "fd 12 was the copy: the close succeeded, its loss is the restore's problem: stderr={err:?}"
    );
}

/// A failing redirection open fails the command before its handler runs: the
/// `cd` does not happen. Bash reports the failure, the command lands at rc 1,
/// and `pwd` prints the unchanged cwd (task #179, bash parity).
#[test]
fn failing_redirect_open_fails_the_command_before_the_handler() {
    let (out, err, code) = run("cd /tmp > /nope-dir-x/f; pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("redirection failed"), "stderr={err:?}");
    assert_ne!(
        out.as_str(),
        "/tmp\n",
        "the cd did not happen: stdout={out:?}"
    );
    assert!(
        !out.is_empty(),
        "pwd printed the unchanged cwd: stdout={out:?}"
    );
}

/// The same for a function call: the open failure fails the call, the body
/// never runs, and the shell continues (task #179, bash parity).
#[test]
fn failing_redirect_open_skips_the_function_body() {
    let (out, err, code) = run("f(){ echo x; }; f > /nope-dir-x/f; echo after");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "after\n", "the body was skipped: stdout={out:?}");
    assert!(err.contains("redirection failed"), "stderr={err:?}");
}

/// A handler that returns a non-zero status is a success path, so the scope
/// restores: fd 1 is back for the next command, and the `return` status
/// survives the call.
#[test]
fn scoped_restore_survives_a_nonzero_handler_status() {
    let (a, _sa) = temp_path("nonzero_a");
    let (out, err, code) = run(&format!(
        "f(){{ return 3; }}; f > {a}; echo after; f > {a}; true"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "after\n", "stdout={out:?}");
}
#[test]
fn capture_on_in_process_builtin_stays_rejected() {
    let (a, _sa) = temp_path("capture_reject");
    let (out, err, code) = run(&format!("read %>%x > {a}"));
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("captures are not supported"), "stderr={err:?}");
}

/// A function call forks, so its capture and its redirect both work.
#[test]
fn capture_on_function_call_is_accepted() {
    let (a, _sa) = temp_path("capture_call");
    let (out, err, code) = run(&format!("f(){{ echo x; }}; f %>%x > {a}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "x\n");
}

/// POSIX #2.2 + #2.4: a clobber redirect applies to the function call (and
/// bypasses `noclobber`), and the scope restore puts the shell's stdout back so
/// the next command writes to its own target. bash 5.3.9 measures `q1` = `in`,
/// `q2` = `after`.
#[test]
fn clobber_applies_to_the_call_and_the_scope_restores() {
    let (q1, _sq1) = temp_path("clobber_call_q1");
    let (q2, _sq2) = temp_path("clobber_call_q2");
    std::fs::write(&q1, b"old1\n").unwrap();
    std::fs::write(&q2, b"old2\n").unwrap();
    let (out, err, code) = run(&format!(
        "set -o noclobber; g(){{ echo in; }}; g >| {q1}; echo after >| {q2}"
    ));
    let first = std::fs::read_to_string(&q1).unwrap();
    let second = std::fs::read_to_string(&q2).unwrap();
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    assert_eq!(first, "in\n", "the call's clobber target");
    assert_eq!(second, "after\n", "the next command's clobber target");
}

/// A clobber write and a `N>&-` close on the same call: the write applies for
/// the call, fd 3 is closed inside it, and the restore reopens fd 3 on its
/// original file afterwards.
#[test]
fn clobber_call_restores_the_closed_fd_target() {
    let (in_path, _sinpath) = temp_path("clobber_closed_in");
    let (q, _sq) = temp_path("clobber_closed_q");
    write_file(&in_path, "hello\n");
    std::fs::write(&q, b"old\n").unwrap();
    let (out, err, code) = run(&format!(
        "set -o noclobber; exec 3< {in_path}; g(){{ echo x; }}; g >| {q} 3>&-; cat <&3; echo after"
    ));
    let body = std::fs::read_to_string(&q).unwrap();
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hello\nafter\n", "stdout={out:?}");
    assert_eq!(body, "x\n");
}
