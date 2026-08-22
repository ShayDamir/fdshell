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
fn unquoted_expansion_splits_on_default_ifs() {
    let (out, err, code) = run(r#"x="a b"; printf %s\n $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn word_splitting_collapses_whitespace_runs() {
    let (out, err, code) = run(r#"x="  a  b "; printf %s\n $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn custom_ifs_delimits_fields() {
    let (out, err, code) = run(r"IFS=:; x=a:b; printf %s\n $x");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn custom_ifs_keeps_empty_fields() {
    let (out, err, code) = run(r#"IFS=:; x="a::b"; printf %s\n $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\n\nb\n");
}

#[test]
fn empty_ifs_disables_word_splitting() {
    let (out, err, code) = run(r#"x="a b"; IFS=; printf %s\n $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\n");
}

#[test]
fn quoted_expansion_does_not_split() {
    let (out, err, code) = run(r#"x="a b"; printf %s\n "$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\n");
}

#[test]
fn unquoted_dollar_at_splits_positional_args() {
    let (out, err, code) = run(r"set -- a b; printf %s\n $@; set --");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn set_dash_dash_splits_expansion() {
    let (out, err, code) = run(r#"x="a b"; set -- $x; printf "%s|%s" $0 $1"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a|b");
}
