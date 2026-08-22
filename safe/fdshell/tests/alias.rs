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
fn alias_expands_command_word() {
    let (out, err, code) = run(r#"alias ll="echo aliased"; ll"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "aliased\n");
}

#[test]
fn alias_replaces_with_command_and_keeps_args() {
    let (out, err, code) = run(r#"alias x=echo; x hello world"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hello world\n");
}

#[test]
fn aliases_chain() {
    let (out, err, code) = run(r#"alias a=b; alias b="echo deep"; a"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "deep\n");
}

#[test]
fn alias_lists_definitions() {
    let (out, err, code) = run(r#"alias zz="echo z"; alias aa="echo a"; alias"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.contains("alias aa='echo a'"), "stdout={out:?}");
    assert!(out.contains("alias zz='echo z'"), "stdout={out:?}");
}

#[test]
fn alias_displays_single_definition() {
    let (out, err, code) = run(r#"alias ll="echo aliased"; alias ll"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "alias ll='echo aliased'\n");
}

#[test]
fn unalias_removes_expansion() {
    let (out, err, code) = run(r#"alias x="echo via"; x; unalias x; x"#);
    assert_ne!(code, 0);
    assert!(err.contains("not found"), "stderr={err:?}");
    assert_eq!(out, "via\n");
}

#[test]
fn expand_aliases_gate_disables_expansion() {
    let (_out, err, code) = run(r#"alias x="echo via"; shopt -u expand_aliases; x"#);
    assert_ne!(code, 0);
    assert!(err.contains("not found"), "stderr={err:?}");
}

#[test]
fn alias_after_leading_whitespace() {
    let (out, err, code) = run("alias ll=\"echo aliased\";    ll");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "aliased\n");
}

#[test]
fn reserved_words_cannot_be_aliased() {
    let (out, err, code) = run(r#"alias if="echo nope"; if true; then echo ok; fi"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "ok\n");
}

#[test]
fn alias_of_unknown_name_errors() {
    let (_out, err, code) = run("alias nosuch");
    assert_ne!(code, 0);
    assert!(err.contains("nosuch"), "stderr={err:?}");
    let (_out, err, code) = run("unalias nosuch");
    assert_ne!(code, 0);
    assert!(err.contains("nosuch"), "stderr={err:?}");
}

#[test]
fn alias_value_with_quote_round_trips() {
    let (out, err, code) = run(r#"alias q="echo a'b"; alias q"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "alias q='echo a'\\''b'\n");
}

#[test]
fn alias_expands_after_pipe() {
    let (out, err, code) = run(r#"alias g="echo got"; echo hi | g"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "got\n");
}

#[test]
fn alias_expands_in_every_pipeline_segment() {
    // The first segment's stdout flows into the pipe; an unexpanded `g` there
    // would surface as "not found" on stderr.
    let (out, err, code) = run(r#"alias g="echo got"; g x | g y"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.is_empty(), "stderr={err:?}");
    assert_eq!(out, "got y\n");
}

#[test]
fn alias_expands_after_pipe_and_cond_operators() {
    let (out, err, code) = run(r#"alias g="echo got"; g x && g y | g z"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.is_empty(), "stderr={err:?}");
    assert_eq!(out, "got x\ngot z\n");
}

#[test]
fn alias_chains_after_pipe() {
    let (out, err, code) = run(r#"alias a=b; alias b="echo deep"; a | b"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.is_empty(), "stderr={err:?}");
    assert_eq!(out, "deep\n");
}

#[test]
fn alias_ignores_pipe_inside_quotes() {
    let (out, err, code) = run(r#"alias g="echo got"; echo "a|b" | g"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "got\n");
}
