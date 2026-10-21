#![allow(clippy::unwrap_used)]

//! `unset name…` (task #290): the operands are **shell variable names**;
//! the `%` spelling keeps fdshell's fd namespace. Every bash column below was
//! measured with `bash -c` (5.3.9) and every fdshell column with
//! `target/debug/fdshell -c` on this task's branch before the rows were pinned:
//!
//! ```text
//! unset v                 bash rc 0 (v gone)      fdshell rc 0 (v gone)
//! unset v w               bash rc 0 (both gone)   fdshell rc 0
//! unset v; echo hi        bash `hi`               fdshell `hi`   (the statement survives)
//! unset                   bash rc 0, `$_`=unset   fdshell rc 0, `$_`=unset
//! v=q; unset v; ${v:-d}   bash `[d]`              fdshell `[d]`
//! unset no_such           bash rc 0, silent       fdshell rc 0, silent
//! unset -v v              bash rc 0, unsets v     fdshell rc 0, unsets v (`-v` names nothing)
//! unset 1 / - / 1abc / a-b / "" / '$#'
//!                           bash rc 0, silent     fdshell rc 0, silent (no identifier check)
//! unset -x                bash rc 2 `invalid option`   fdshell rc 0  ← divergence
//! set -- x y; unset 1; $1 bash `x`                fdshell `x`
//! unset a b; echo $_      bash `b`                fdshell `b`
//! unset %v                bash rc 0 (a plain name)fdshell rc 0 (closes fd `%v`)  ← namespaces
//! f(){ :; }; unset f; f   bash `command not found` rc 127   fdshell runs f, rc 0  ← task #386
//! ```

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");
const HELPER: &str = env!("EXEC_OK_PATH");

/// The helper dir first, then the ambient `PATH` (NixOS has no `/usr/bin`), so
/// the child-side `env`/`grep` of the environment rows resolve (the
/// `tests/cd_expand.rs` shape).
fn test_path() -> String {
    let path_dir = HELPER
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("/usr/bin");
    let ambient = std::env::var("PATH").unwrap_or_default();
    format!("{path_dir}:{ambient}")
}

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

/// `run` with the child's environment spelled out pair by pair (the shape of
/// `tests/cd_expand.rs::run_env`): a `"\0"` value **removes** the key, the only
/// way to hand the child no `HOME` at all. Every pair is applied over the
/// harness's own environment, which is what makes an *inherited* name the thing
/// `unset` has to take away.
fn run_env(env: &[(&str, &str)], script: &str) -> (String, String, i32) {
    let mut cmd = Command::new(BIN);
    cmd.args(["-c", script])
        .env("PATH", test_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        if *value == "\0" {
            cmd.env_remove(key);
        } else {
            cmd.env(key, value);
        }
    }
    let output = cmd.output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// A per-test scratch dir, removed on drop (the sweep-safe shape of
/// `tests/cd_expand.rs::Scratch`).
struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn scratch(tag: &str) -> Scratch {
    let dir = std::env::temp_dir().join(format!(
        "fdshell-unset-{tag}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// The task's own table: a plain name names a shell variable, several operands
/// are applied left to right, and — the part that made this a bug — a parse
/// error used to kill the **whole statement**, so `unset v; echo hi` printed
/// nothing.
#[test]
fn plain_names_are_shell_variables() {
    for script in [
        "v=1; unset v; builtin echo [${v:-U}]",
        "v=1; w=2; unset v w; builtin echo [${v:-U}][${w:-U}]",
        "v=q; unset v; builtin printf \"[%s]\" \"${v:-d}\"",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "script {script:?}: stderr={err:?}");
        assert_eq!(out, "[U]\n", "script {script:?}");
    }
    let (out, err, code) = run("unset v; builtin echo hi");
    assert_eq!(code, 0, "the statement must survive: stderr={err:?}");
    assert_eq!(out, "hi\n");
}

/// No operands is a no-op at rc 0 (bash), and `$_` falls back to the command
/// word — the same fallback fdshell already has for `true`.
#[test]
fn bare_unset_is_a_noop() {
    let (out, err, code) = run("v=1; unset; builtin echo [${v:-U}]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[1]\n");
    let (out, err, code) = run("true a; unset; builtin echo [$_]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[unset]\n", "bash measures `[unset]`");
}

/// An unbound name is silent at rc 0: there is no error path for a name absent
/// from a store (bash 5.3.9, measured).
#[test]
fn unbound_names_are_silent() {
    for script in [
        "unset no_such_name",
        "unset no_such_name other_name",
        "unset %no_such_fd",
        "unset %arr[%no_such_fd]",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "script {script:?}: stderr={err:?}");
        assert!(out.is_empty(), "script {script:?}: stdout={out:?}");
    }
}

/// A shell-variable `unset` takes the name out of **every** store that holds
/// it: the value, the export attribute, and the child environment. bash 5.3.9
/// measured the same, both for a name the shell created and for an inherited
/// one (`HOME` is the inherited name the `cd` rows below depend on).
#[test]
fn export_and_the_environment_lose_the_name() {
    let (out, err, code) = run("v=1; export v; unset v; env | grep ^v=");
    assert_eq!(code, 1, "grep finds no line: stdout={out:?} {err:?}");
    assert!(out.is_empty(), "stdout={out:?}");

    let home = scratch("env-home");
    let inherited = home.display().to_string();
    let env = [("HOME", inherited.as_str())];

    // The inherited row answers `$HOME` / `${HOME:-x}` (resolve::var_value) and
    // is exported by exec::environ::get_environ; only removing it from
    // `state.environ` takes the name away from both.
    let (out, err, code) = run_env(&env, "unset HOME; builtin echo [${HOME:-U}]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[U]\n");
    let (out, err, code) = run_env(&env, "unset HOME; env | grep ^HOME=");
    assert_eq!(code, 1, "the child env lost HOME: stdout={out:?} {err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
}

/// The `environ` half is what makes the bare `cd` fail after `unset HOME`:
/// the target lookup (`ShellState::var_value`) sees no `HOME` at all, and
/// fdshell reports rc 1 with the same words bash uses (`cd: HOME not set`).
#[test]
fn cd_after_unset_home_reports_home_not_set() {
    let home = scratch("cd-home");
    let inherited = home.display().to_string();
    let (out, err, code) = run_env(&[("HOME", inherited.as_str())], "unset HOME; cd");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    assert!(err.contains("HOME not set"), "stderr={err:?}");
}

/// `unset IFS` restores the **default** split set (space, tab, newline), it
/// does not leave the old separators in force; `$IFS` itself reads empty.
#[test]
fn unset_ifs_restores_the_default_split_set() {
    // A comma no longer splits…
    let (out, err, code) = run(
        "IFS=,; set -- a,b; unset IFS; for w in $1; do builtin echo \"[$w]\"; done",
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a,b]\n");
    // …and a space splits again.
    let (out, err, code) =
        run("IFS=,; unset IFS; set -- \"a b\"; for w in $1; do builtin echo \"[$w]\"; done");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a]\n[b]\n");
    let (out, err, code) = run("IFS=,; unset IFS; builtin echo \"[$IFS]\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n");
}

/// After the unset the name is **unbound**, which `set -u` reports — for a name
/// the shell created and for an inherited one (the second row is the `environ`
/// half again). fdshell's rc is its shell-wide 1, not bash's 127 (the accepted
/// rc-1-vs-127 family).
#[test]
fn nounset_reports_the_name_unbound() {
    let (out, err, code) = run("set -u; v=1; unset v; builtin echo $v");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    assert!(err.contains("v: unbound variable"), "stderr={err:?}");

    let (out, err, code) = run_env(
        &[("ZZ", "1")],
        "set -u; unset ZZ; builtin echo $ZZ",
    );
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    assert!(err.contains("ZZ: unbound variable"), "stderr={err:?}");
}

/// bash 5.3.9 validates **no** operand of `unset`: `1`, `-`, `1abc`, `a-b`,
/// `$#`, `*`, `-v` are ordinary words, and the ones naming no variable are
/// silent no-ops. `unset 1` never touches `$1` (measured in both shells), and
/// `unset -v v` unsets `v` because `-v` is a word naming nothing. The one
/// measured divergence is option *validation*: `unset -x` is bash's rc-2
/// `invalid option` and fdshell's rc 0 (POSIX_COMPATIBILITY.md, task #387).
#[test]
fn every_operand_shape_bash_accepts_is_a_plain_name() {
    for script in [
        "unset 1",
        "unset -",
        "unset 1abc",
        "unset a-b",
        "unset '$#'",
        "unset '*'",
        "unset -x",
        "unset \"\"",
        "unset \"a b\"",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "script {script:?}: stderr={err:?}");
        assert!(out.is_empty(), "script {script:?}: stdout={out:?}");
    }

    let (out, err, code) = run("set -- x y; unset 1; builtin echo $1");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "x\n");
    let (out, err, code) = run("v=1; unset -v v; builtin echo [${v:-U}]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[U]\n");
}

/// Function frames need no change: `local v` records its first-touch value, so
/// `unset v` inside a call leaves the name unset **for the call** and the
/// caller's value comes back at return (bash's rows, measured).
#[test]
fn a_function_frame_restores_the_callers_value() {
    let (out, err, code) = run(
        "v=pre; f(){ local v=1; unset v; builtin echo ${v:-U}; }; f; builtin echo ${v:-U}",
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "U\npre\n");
}

/// `$_` is the **last operand's** stored name: the word as written for a shell
/// variable, `%`-stripped for an fd one (the rule
/// `tests/last_arg.rs::unset_updates_last_arg` pins), and the array name for
/// `%arr[%src]`.
#[test]
fn last_arg_is_the_last_operand_name() {
    let (out, err, code) = run("true seed; unset a b; builtin echo [$_]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[b]\n", "bash measures `[b]`");
    let (out, err, code) = run("true seed; unset v %f; builtin echo [$_]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[f]\n");
    let (out, err, code) = run("true seed; unset %a[%c]; builtin echo [$_]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a]\n");
}

/// The namespaces stay separate (`%` = fd, STYLE §5): a plain-name `unset`
/// closes no descriptor, and the fd form still closes the descriptor, drops an
/// fd array of that name and removes a single array entry.
#[test]
fn the_fd_namespace_stays_the_fd_namespace() {
    // A plain-name unset of the *same* name leaves the descriptor alone.
    let (out, err, code) = run(
        "builtin openat2 --flags O_RDONLY /dev/null %>%x; x=1; unset x; builtin statx %x; builtin echo [${x:-U}]",
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.starts_with("kind=char"), "stdout={out:?}");
    assert!(out.ends_with("[U]\n"), "stdout={out:?}");

    // `%name` closes the descriptor: the next use reports the fd variable as
    // unset (the `tests/statx.rs` shape) at rc 1.
    let (out, err, code) = run(
        "builtin openat2 --flags O_RDONLY /dev/null %>%f; unset %f; builtin statx %f; builtin echo after",
    );
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty(), "the run aborts: stdout={out:?}");
    assert!(err.contains("fd variable"), "stderr={err:?}");

    // One array entry goes, the array stays; the whole array goes with `%name`.
    let (out, err, code) = run(
        "builtin pipe %rd>%r %wr>%w; %a+=%r; %a+=%w; unset %a[%r]; for %x in %a; do builtin echo x; done; unset %a; builtin echo empty; for %x in %a; do builtin echo y; done",
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "x\nempty\ny\n", "one entry, then the array is gone");
}

/// **Documented divergence** (task #386): bash's `unset NAME` also removes a
/// *function* named `NAME` (`f(){ :; }; unset f; f` → `command not found`, rc
/// 127). fdshell has no `-f`/`-v` option surface and no function-removal path,
/// so the function survives; pinned as measured so the follow-up sees the row.
#[test]
fn a_function_survives_unset_documented_divergence() {
    let (out, err, code) = run("f(){ builtin echo F; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "F\n");
    let (out, err, code) = run("f(){ builtin echo F; }; unset f; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "F\n", "task #386: bash prints nothing and exits 127");
}
