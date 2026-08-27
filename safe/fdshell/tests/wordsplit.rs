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

fn run_stdin(script: &str, stdin: &str) -> (String, String, i32) {
    use std::io::Write;
    let mut child = Command::new(BIN)
        .args(["-c", script])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.stdin.take().unwrap();
    let output = child.wait_with_output().unwrap();
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

#[test]
fn read_ifs_updates_word_splitting() {
    let (out, err, code) = run_stdin(r"read IFS; x=a:b; printf %s\n $x", ":\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn export_ifs_updates_word_splitting() {
    let (out, err, code) = run(r"export IFS=:; x=a:b; printf %s\n $x");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn param_op_ifs_assign_updates_word_splitting() {
    let (out, err, code) = run(r"IFS=; y=a,b; x=${IFS:=,}; printf %s\n $y");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn for_ifs_updates_word_splitting() {
    let (out, err, code) = run(r"for IFS in ,; do x=a,b; printf %s\n $x; done");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn unquoted_dollar_at_custom_ifs_splits_per_positional() {
    let (out, err, code) = run(r"IFS=:; set -- a:b c; printf %s\n $@");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\nc\n");
}

#[test]
fn unquoted_dollar_at_empty_ifs_keeps_positionals() {
    let (out, err, code) = run(r#"set -- "a b" c; IFS=; printf %s\n $@"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\nc\n");
}

#[test]
fn quoted_dollar_star_custom_ifs_joins_with_first_ifs_byte() {
    let (out, err, code) = run(r#"IFS=:; set -- a b; printf %s\n "$*""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a:b\n");
}

#[test]
fn quoted_dollar_star_empty_ifs_joins_with_nothing() {
    let (out, err, code) = run(r#"IFS=; set -- a b; printf %s\n "$*""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "ab\n");
}

#[test]
fn embedded_dollar_at_uses_first_ifs_byte_join() {
    let (out, err, code) = run(r"IFS=:; set -- a b; printf %s\n x$@");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "xa\nb\n");
}

#[test]
fn mixed_quoted_token_stays_one_word() {
    // TODO.md regression: quoted IFS inside a mixed token used to split the
    // word (one argv entry silently became two).
    let (out, err, code) = run(r#"builtin printf "[%s]" x"a b"c"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[xa bc]");
}

#[test]
fn mixed_quoted_expansion_stays_one_word() {
    let (out, err, code) = run(r#"x="a b"; builtin printf "[%s]" y"$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[ya b]");
}

#[test]
fn mixed_unquoted_expansion_still_splits() {
    let (out, err, code) = run(r#"x="a b"; builtin printf "[%s]" y$x z"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[ya][b][z]");
}

#[test]
fn mixed_quoted_non_whitespace_ifs_protected() {
    let (out, err, code) = run(r#"IFS=:; x=a:b; builtin printf "[%s]" y"$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[ya:b]");
}

#[test]
fn quoted_ifs_after_cmd_subst_stays_one_word() {
    // The quoted `:` after the substitution must keep its protection even
    // though the `$( )` span consumed several input bytes.
    let (out, err, code) = run(r#"IFS=:; builtin printf "[%s]" x$(true)":z""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[x:z]");
}

#[test]
fn unquoted_ifs_after_cmd_subst_still_splits() {
    let (out, err, code) = run(r#"IFS=:; builtin printf "[%s]" x$(true):y"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[x][y]");
}

#[test]
fn quoted_middle_of_word_keeps_ifs_and_splits_around_it() {
    // Unquoted IFS outside the quotes still delimits; quoted IFS stays.
    let (out, err, code) = run(r#"x="a b"; builtin printf "[%s]" "$x" c"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a b][c]");
}
