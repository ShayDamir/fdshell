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
fn scoped_prefix_is_not_persisted() {
    let (out, err, code) = run("FOO=bar true; builtin echo ${FOO:-unset}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "unset\n");
}

#[test]
fn scoped_prefix_is_exported_to_external_child() {
    // Unquoted grep pattern: quoted args in pipeline stages are a separate
    // pre-existing issue, not part of this task.
    let (out, err, code) = run("FOO=bar env | grep ^FOO=");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "FOO=bar\n");
}

#[test]
fn bare_compound_assignment_persists() {
    let (out, err, code) = run("FOO=bar BAZ=qux; builtin echo $FOO$BAZ");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "barqux\n");
}

#[test]
fn first_touch_value_is_restored() {
    // Bash restores the value from before the prefix: `FOO=old FOO=new`
    // records the original once, so the shell's `FOO=pre` comes back.
    let (out, err, code) = run("FOO=pre; FOO=old FOO=new cd /tmp; builtin echo $FOO");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "pre\n");
}

#[test]
fn first_touch_of_unset_var_restores_unset() {
    let (out, err, code) = run("FOO=old FOO=new cd /tmp; builtin echo ${FOO:-unset}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "unset\n");
}

#[test]
fn scoped_prefix_does_not_clear_last_arg() {
    let (out, err, code) = run("FOO=bar true hi; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn prefix_value_is_expanded_against_shell_state() {
    let (out, err, code) = run("A=1; FOO=$A true; builtin echo ${FOO:-unset}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "unset\n");
    let (out, err, code) = run("A=1; FOO=$A env | grep ^FOO=");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "FOO=1\n");
}

#[test]
fn scoped_ifs_does_not_split_the_commands_own_words() {
    let (out, err, code) = run("IFS=: builtin echo a:b");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a:b\n");
}

#[test]
fn scoped_ifs_is_restored_afterward() {
    let (out, err, code) = run("IFS=: true; x=a:b; builtin echo $x");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a:b\n");
}

#[test]
fn scoped_ifs_is_visible_to_the_command() {
    // The external child sees the scoped IFS in its environment.
    let (out, err, code) = run("IFS=: env | grep ^IFS=");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "IFS=:\n");
}

#[test]
fn scoped_prefix_is_visible_inside_a_function() {
    let (out, err, code) =
        run("f() { builtin echo in=$FOO; }; FOO=bar f; builtin echo out=${FOO:-unset}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "in=bar\nout=unset\n");
}

#[test]
fn bare_assignment_separated_by_semicolon_persists() {
    let (out, err, code) = run("FOO=bar; builtin echo $FOO");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "bar\n");
}

#[test]
fn scoped_prefix_before_pipeline_is_a_parse_error() {
    let (_out, err, code) = run("FOO=bar | cat");
    assert_ne!(code, 0);
    assert!(!err.is_empty(), "expected a parse error on stderr");
}

#[test]
fn scoped_prefix_before_keyword_runs_keyword_as_command() {
    // Divergence from bash: `if` becomes a plain (not found) command.
    let (_out, err, code) = run("FOO=bar if true; then :; fi");
    assert_ne!(code, 0);
    assert!(
        err.contains(r#"failed to resolve command path: "if""#),
        "stderr={err:?}"
    );
}

#[test]
fn word_with_equals_after_command_is_an_arg() {
    let (out, err, code) = run("true FOO=bar; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "FOO=bar\n");
}

#[test]
fn scoped_prefix_on_intercepted_command_is_visible() {
    let (out, err, code) = run("FOO=bar eval \"builtin echo $FOO\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "bar\n");
    let (out, err, code) = run("FOO=bar eval \"builtin echo $FOO\"; builtin echo ${FOO:-unset}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "bar\nunset\n");
}
