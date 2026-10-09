#![allow(clippy::unwrap_used)]

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// `fdshell -c script` → (stdout, stderr, exit code). Every expectation below
/// was verified against bash on this machine first.
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

/// A script file in the temp dir, removed when the guard drops.
struct TempScript(String);

impl TempScript {
    fn new(name: &str, content: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("fdshell-it-local-{name}-{}", std::process::id()));
        std::fs::write(&path, content).unwrap();
        Self(path.to_str().unwrap().to_string())
    }
}

impl Drop for TempScript {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn local_variable_does_not_leak_into_the_caller() {
    let (out, err, code) = run("f(){ local x=1; }; f; echo \"${x:-unset}\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "unset\n");
}

#[test]
fn local_is_visible_inside_the_call_and_restored_after() {
    let (out, err, code) = run("v=0; f(){ local v=42; echo $v; }; f; echo $v");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "42\n0\n");
}

#[test]
fn declare_form_leaves_the_variable_unset_for_the_call() {
    // `${v+set}` is not fdshell syntax, so `${v:-unset}` shows the unset state.
    let (out, err, code) = run("v=pre; f(){ local v; echo \"${v:-unset}\"; }; f; echo $v");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "unset\npre\n");
}

#[test]
fn nested_calls_have_their_own_frames() {
    let (out, err, code) = run(
        "n(){ local z=outer; m(){ local z=inner; echo $z; }; m; echo $z; }; n; echo \"${z:-unset}\"",
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "inner\nouter\nunset\n");
}

#[test]
fn recursion_gives_each_call_its_own_frame() {
    // bash: 0, 1, 2. A cond list (`&&`) is not parsed inside a function body
    // (task #168), so the base case uses `if`.
    let (out, err, code) =
        run("r(){ local r=$1; if [ \"$1\" -gt 0 ]; then r $((r-1)); fi; echo $r; }; r 2");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "0\n1\n2\n");
}

#[test]
fn first_touch_restores_the_caller_value_after_two_locals() {
    let (out, err, code) = run("v=pre; f(){ local v=1; local v=2; echo $v; }; f; echo $v");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "2\npre\n");
}

#[test]
fn a_local_is_exported_to_children_inside_the_call() {
    let (out, err, code) =
        run("export E=env; f(){ local E=loc; env | grep ^E=; }; f; env | grep ^E=");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "E=loc\nE=env\n");
}

#[test]
fn a_declared_local_keeps_the_caller_export_visible() {
    // bash: the export attribute is global, so `local E` leaves `E=env` in the
    // child environment.
    let (out, err, code) = run("export E=env; f(){ local E; env | grep ^E=; }; f; env | grep ^E=");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "E=env\nE=env\n");
}

#[test]
fn local_ifs_splits_words_inside_the_call_and_is_restored_after() {
    let (out, err, code) = run("x=\"a:b c\"; f(){ local IFS=:; echo $x; }; f; echo $x");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b c\na:b c\n");
}

#[test]
fn the_value_is_expanded_without_splitting_or_globbing() {
    // The stored value is the raw `*` (no pathname expansion at assignment),
    // and a `$v` value keeps its space (no field splitting).
    let (out, err, code) = run("f(){ local x=*; echo \"$x\"; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "*\n");
    let (out, err, code) = run("v=\"a b\"; f(){ local v2=$v; echo \"[$v2]\"; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a b]\n");
    let (out, err, code) = run("f(){ local q=\"a b\"; echo \"[$q]\"; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a b]\n");
}

#[test]
fn the_list_form_prints_the_call_locals_sorted() {
    // POSIX/dash form `local NAME=value`; bash prints `declare -- NAME="value"`.
    let (out, err, code) = run("f(){ local q=2 p=1; local; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "local p=1\nlocal q=2\n");
}

#[test]
fn a_declared_local_lists_without_a_value() {
    let (out, err, code) = run("f(){ local p; local; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "local p\n");
}

#[test]
fn local_outside_a_function_is_an_error() {
    let (_out, err, code) = run("local o=1");
    assert_eq!(code, 1);
    assert!(err.contains("function"), "stderr={err:?}");
}

#[test]
fn a_local_in_a_sourced_file_at_top_level_is_an_error() {
    let script = TempScript::new("toplevel", "local x=1\n");
    let (_out, err, code) = run(&format!("source {}", script.0));
    assert_eq!(code, 1);
    assert!(err.contains("function"), "stderr={err:?}");
}

#[test]
fn an_fd_var_name_is_not_a_valid_local_name() {
    // fd scoping is task #46; `local %x` is rejected, not a silent no-op.
    let (_out, err, code) = run("f(){ local %x=1; }; f");
    assert_eq!(code, 1);
    assert!(err.contains("not a valid"), "stderr={err:?}");
}

#[test]
fn an_option_word_is_rejected_as_a_name() {
    // Accepted divergence: bash takes `local -x` as its own option (rc 0 on
    // this machine); fdshell rejects the word as a name (rc 1).
    let (_out, _err, code) = run("f(){ local -x; }; f");
    assert_eq!(code, 1);
}

#[test]
fn restore_is_visible_in_the_set_listing() {
    let (out, err, code) = run("v=pre; f(){ local v=1; }; f; set");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.lines().any(|l| l == "v=pre"), "stdout={out:?}");
    assert!(!out.lines().any(|l| l == "v=1"), "stdout={out:?}");
}

#[test]
fn return_keeps_its_status_and_restores_the_frame() {
    let (out, err, code) = run("v=pre; f(){ local v=1; return 3; }; f; echo $?; echo $v");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "3\npre\n");
}

#[test]
fn local_works_through_eval_inside_a_call() {
    // Single quotes are literal bytes in fdshell, so the eval word is double-quoted.
    let (out, err, code) = run("f(){ eval \"local z=1\"; echo $z; }; f; echo \"${z:-unset}\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\nunset\n");
}

#[test]
fn a_fork_inside_a_call_accepts_local() {
    // Accepted divergence: the frame is inherited by the `$(…)` child, so
    // `local` works inside a command substitution; bash refuses in a subshell.
    let (out, err, code) = run("f(){ x=$(local y=7; echo $y); echo $x; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "7\n");
}

#[test]
fn builtin_and_command_prefixes_run_local() {
    let (out, err, code) = run("f(){ builtin local v=1; echo $v; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n");
    let (out, err, code) = run("f(){ command local v=1; echo $v; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n");
}

#[test]
fn the_command_word_folds_its_escape_pair() {
    // POSIX #4.1: `lo\cal` folds to `local` at word 0, so the builtin runs.
    let (out, err, code) = run("f(){ lo\\cal v=1; echo $v; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n");
}

#[test]
fn xtrace_prints_the_local_command() {
    let (_out, err, code) = run("set -x; f(){ local v=1; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ local v=1"), "stderr={err:?}");
}

#[test]
fn a_user_function_named_local_shadows_the_builtin() {
    // fdshell resolves functions before intercepts (its standing convention).
    let (out, err, code) = run("local(){ echo shadow; }; f(){ local; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "shadow\n");
}

#[test]
fn scoping_is_dynamic() {
    // bash: `g` sees the caller-call's local `v`, since `g` does not shadow it.
    let (out, err, code) = run("v=1; g(){ echo $v; }; f(){ local v=2; g; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "2\n");
}

#[test]
fn a_scoped_prefix_restores_under_the_local_frame() {
    // `v=mid` applies for the whole call, `local` restores `mid` on return,
    // then the prefix restore puts back the caller's `v`.
    let (out, err, code) = run("v=pre; f(){ local v=1; echo $v; }; v=mid f; echo $v");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\npre\n");
}

#[test]
fn help_lists_local() {
    let (out, err, code) = run("help");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        out.lines()
            .any(|l| l.starts_with("local") && l.contains("Declare function-scoped variables")),
        "stdout={out:?}"
    );
}
