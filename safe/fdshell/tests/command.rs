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

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). Removal is `.ok()`-tolerant: teardown never panics, and it
/// runs on a failing test's unwind, which is exactly the test that used to
/// leak.
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

/// Write an executable `#!/bin/sh` script named `name` in a fresh temp dir.
fn make_external(tag: &str, name: &str, body: &str) -> (std::path::PathBuf, Scratch) {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("command_{tag}_{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join(name);
    std::fs::write(&exe, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    (exe, Scratch(dir))
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
    let (exe, _scratch) = make_external("ext", "cmd_ext_xyz", "#!/bin/sh\necho EXTERNAL\n");
    let dir = exe.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(&path_with(dir), "command cmd_ext_xyz");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "EXTERNAL\n");
}

#[test]
fn command_runs_external_with_args() {
    let (exe, _scratch) = make_external(
        "extargs",
        "cmd_ext_xyz",
        "#!/bin/sh\necho EXTERNAL \"$@\"\n",
    );
    let dir = exe.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(&path_with(dir), "command cmd_ext_xyz a b");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "EXTERNAL a b\n");
}

#[test]
fn command_prefers_builtin_over_path_shadow() {
    // `command` implies builtin-first: a PATH shadow of a builtin name is
    // bypassed (unlike a bare name with `builtin_first` off).
    let (exe, _scratch) = make_external("shadow", "echo", "#!/bin/sh\necho SHADOWED \"$@\"\n");
    let dir = exe.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(&path_with(dir), "command echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
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

/// The class rule, pinned (task #473). The prefix is a **lookup-layer** rule: the
/// word behind `builtin`/`command` goes to the name lookups, and the five words
/// `umask`, `unset`, `break`, `continue`, `return` are parse-level keywords with
/// no entry in any name table — so `command NAME` falls through to the `PATH`
/// search and `builtin NAME` answers `is not a shell builtin`, rc 1, in every row
/// below. Measured against bash 5.3.9 (2026-11-10), which reaches its builtin in
/// every row: `umask` prints `0022` at rc 0, `unset x` succeeds at rc 0,
/// `break`/`continue` warn at rc 0 and `return` warns at **rc 2**. The
/// reaches-half row at the end matches bash exactly, so the rule is a lookup rule
/// and not an accident. The quoting side of the same absence is pinned by
/// `tests/quotes.rs::the_five_names_have_no_builtins_pinned`; the decision and its
/// cost are in `POSIX_COMPATIBILITY.md`, "Quoted keywords are command words,
/// quoted control keywords are not".
#[test]
fn prefix_reaches_name_lookups_not_parse_keywords() {
    for (script, needle) in [
        ("command umask", "\"umask\" not found"),
        ("builtin umask", "\"umask\" is not a shell builtin"),
        ("command break", "\"break\" not found"),
        ("builtin break", "\"break\" is not a shell builtin"),
        ("command continue", "\"continue\" not found"),
        ("builtin continue", "\"continue\" is not a shell builtin"),
        ("command unset x", "\"unset\" not found"),
        ("builtin unset x", "\"unset\" is not a shell builtin"),
        ("command return", "\"return\" not found"),
        ("builtin return", "\"return\" is not a shell builtin"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err}");
        assert!(err.contains(needle), "{script}: stderr={err}");
    }
    // The lookup half of the rule: a name a table *does* have is reached by the
    // prefix, rc for rc with bash (`false` via the prefix is rc 1).
    let (out, err, code) = run("builtin false; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=1\n", 0), "stderr={err}");
}

/// What the rule costs on the state-changing words (task #473): the prefix row is
/// the failed lookup above, so the state change never happens. bash column
/// (bash 5.3.9, measured 2026-11-10) — `[gone]` for all four rows, and `0022` for
/// both umask rows (the mask is pinned first, so the row never reads the
/// environment, the `tests/umask.rs` convention). Every report is on stderr in
/// both shells, so the stdout asserted here is uncontaminated; the shell rc is 0
/// because the last statement succeeds.
#[test]
fn a_prefix_row_loses_the_state_change() {
    // bash column: `0022\n` — bash's builtin applied the mask.
    for script in [
        "umask 077; command umask 022; umask",
        "umask 077; builtin umask 022; umask",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            ("0077\n", 0),
            "{script}: stderr={err}"
        );
    }
    // bash column: `[gone]\n` — bash's builtin unset the variable.
    for script in [
        r#"x=1; command unset x; echo "[${x:-gone}]""#,
        r#"x=1; builtin unset x; echo "[${x:-gone}]""#,
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("[1]\n", 0), "{script}: stderr={err}");
    }
}

/// What the rule costs on the loop-control words (task #473): a prefix-qualified
/// `break`/`continue`/`return` raises no in-process signal, because it never
/// reaches the keyword. bash column (bash 5.3.9, measured 2026-11-10): the loop
/// prints **nothing** (`break`/`continue` warn at rc 0) and `f` returns 3, so
/// `echo rc=$?` prints `rc=3`. fdshell answers the lookup at rc 1, runs both
/// iterations printing `after` twice, and ends the function at the failed
/// command's rc — `rc=1`. The shell rc is 0 in both columns.
#[test]
fn a_prefix_row_raises_no_loop_control() {
    // bash column: nothing at rc 0.
    for script in [
        "for i in 1 2; do command break; echo after; done",
        "for i in 1 2; do builtin break; echo after; done",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            ("after\nafter\n", 0),
            "{script}: stderr={err}"
        );
    }
    // bash column: `rc=3\n` — bash's builtin returned from the function.
    for script in [
        "f() { command return 3; }; f; echo rc=$?",
        "f() { builtin return 3; }; f; echo rc=$?",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            ("rc=1\n", 0),
            "{script}: stderr={err}"
        );
    }
}
