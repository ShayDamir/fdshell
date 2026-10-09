#![allow(clippy::unwrap_used)]

use std::process::{Command, Stdio};
use std::str;
use std::sync::atomic::Ordering;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");
static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

/// A scratch dir with `a1 a2 ax foo.bar foo2` (the star/brace cases need real
/// files to prove an escaped pattern byte never globs); each test gets its own.
fn scratch(names: &[&str]) -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-esc-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    for name in names {
        std::fs::write(dir.join(name), b"").unwrap();
    }
    dir
}

fn run_in(dir: &std::path::PathBuf, script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .current_dir(dir)
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

/// POSIX #4.1: the escape pair `\X` is the literal `X` (the backslash is
/// removed), so `a\*` prints `a*` and never globs the `a1`/`a2` files.
#[test]
fn escaped_star_is_a_literal_word() {
    let dir = scratch(&["a1", "a2", "ax", "foo.bar", "foo2"]);
    let (out, _err, code) = run_in(&dir, "echo a\\*");
    assert_eq!(code, 0);
    assert_eq!(out, "a*\n");
    // The escaped byte is mask-protected, so a leading escaped star is data.
    let (out, _err, code) = run_in(&dir, "echo \\*");
    assert_eq!(code, 0);
    assert_eq!(out, "*\n");
    // An escaped prefix leaves the rest of the word globs.
    let (out, _err, code) = run_in(&dir, "echo f\\oo\\.*");
    assert_eq!(code, 0);
    assert_eq!(out, "foo.bar\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The escaped space is one word byte: the pair is not an IFS split point.
#[test]
fn escaped_space_is_one_word() {
    let (out, _err, code) = run("builtin echo [a\\ b]");
    assert_eq!(code, 0);
    assert_eq!(out, "[a b]\n");
    // The escaped space stays inside the value: no word splitting on it.
    let (out, _err, code) = run("X=a\\ b; builtin echo [$X]");
    assert_eq!(code, 0);
    assert_eq!(out, "[a b]\n");
}

/// The escaped separators are word bytes: `;`, `|`, `&` never split the
/// command, and `>` is a word (bash backgrounds `a\&&b` — the accepted
/// divergence, README).
#[test]
fn escaped_separators_are_word_bytes() {
    let (out, _err, code) = run("builtin echo a\\;b");
    assert_eq!(code, 0);
    assert_eq!(out, "a;b\n");
    let (out, _err, code) = run("builtin echo a\\|b | cat");
    assert_eq!(code, 0);
    assert_eq!(out, "a|b\n");
    let (out, _err, code) = run("builtin echo a\\>b");
    assert_eq!(code, 0);
    assert_eq!(out, "a>b\n");
    // A `>` after an escaped `&` is still a redirect (bash: `2>1` fails).
    let (out, _err, code) = run("builtin echo 2\\>&1");
    assert_eq!(code, 0);
    assert_eq!(out, "2>&1\n");
}

/// An escaped `(`/`)` is a word byte: it never opens or closes a subshell,
/// and inside `$( … )` it keeps the body open (bash prints `a)b`).
#[test]
fn escaped_parens_are_word_bytes() {
    let (out, _err, code) = run("builtin echo a\\)b");
    assert_eq!(code, 0);
    assert_eq!(out, "a)b\n");
    let (out, _err, code) = run("builtin echo a\\(b");
    assert_eq!(code, 0);
    assert_eq!(out, "a(b\n");
    // The escaped `)` is data, so the substitution body ends at the real `)`.
    let (out, _err, code) = run("builtin echo $(echo a\\)b)");
    assert_eq!(code, 0);
    assert_eq!(out, "a)b\n");
}

/// An escaped brace is literal: brace expansion never sees a group.
#[test]
fn escaped_braces_are_literal() {
    let (out, _err, code) = run("builtin echo \\{a,b\\}");
    assert_eq!(code, 0);
    assert_eq!(out, "{a,b}\n");
    let (out, _err, code) = run("builtin echo a\\{b,c\\}");
    assert_eq!(code, 0);
    assert_eq!(out, "a{b,c}\n");
}

/// An escaped `#` is a word byte, not a comment; an escaped `~` is a literal
/// tilde; an escaped `\` folds to one backslash.
#[test]
fn escaped_comment_tilde_and_backslash() {
    let (out, _err, code) = run("builtin echo \\#hi");
    assert_eq!(code, 0);
    assert_eq!(out, "#hi\n");
    let (out, _err, code) = run("builtin echo \\~");
    assert_eq!(code, 0);
    assert_eq!(out, "~\n");
    let (out, _err, code) = run("builtin echo a\\\\b");
    assert_eq!(code, 0);
    assert_eq!(out, "a\\b\n");
}

/// `\<newline>` is a line continuation: both bytes are dropped.
#[test]
fn escaped_newline_is_a_line_continuation() {
    let (out, _err, code) = run("printf %s x\\\ny");
    assert_eq!(code, 0);
    assert_eq!(out, "xy");
}

/// The escaped `$` and backtick are literal: no expansion, no command
/// substitution.
#[test]
fn escaped_dollar_and_backtick_are_literal() {
    let (out, _err, code) = run("X=hi; builtin echo a\\$X");
    assert_eq!(code, 0);
    assert_eq!(out, "a$X\n");
    let (out, _err, code) = run("printf %s a\\`b");
    assert_eq!(code, 0);
    assert_eq!(out, "a`b");
}

/// A trailing `\` at end of input keeps its backslash (bash prints `[a\]`).
#[test]
fn trailing_backslash_is_kept() {
    let (out, _err, code) = run("printf \"[%s]\" a\\");
    assert_eq!(code, 0);
    assert_eq!(out, "[a\\]");
}

/// POSIX #4.2 is unchanged: inside double quotes the pair is kept for the
/// builtin, so `printf` still interprets `\n`.
#[test]
fn quoted_escape_rules_are_unchanged() {
    let (out, _err, code) = run("printf \"a\\*b\"");
    assert_eq!(code, 0);
    assert_eq!(out, "a\\*b");
    let (out, _err, code) = run("printf \"%s\n\" hello");
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
    let (out, _err, code) = run("builtin echo \"a\\\"b\"; builtin echo hi");
    assert_eq!(code, 0);
    assert_eq!(out, "a\"b\nhi\n");
}

/// A delimiter word folds its unquoted escape pairs (`<<E\OF` delimits `EOF`),
/// a quoted delimiter keeps the pair, and `echo a\<<X` is one word.
#[test]
fn heredoc_delimiter_folds_escape_pairs() {
    let (out, err, code) = run("cat <<E\\OF\nbody\nEOF\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "body\n");
    // A quoted delimiter: the pair is literal, so the terminator is `E\OF`.
    let (out, err, code) = run("cat <<\"E\\OF\"\nbody\nE\\OF\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "body\n");
    // The escape pair `\<` folds to a literal `<`, so no operator is formed.
    let (out, _err, code) = run("printf %s a\\<<X");
    assert_eq!(code, 0);
    assert_eq!(out, "a<<X");
}

/// An escape pair in the name makes the word a plain command word: `\X=1`
/// runs the command `X=1` (not found), never an assignment.
#[test]
fn escaped_name_is_not_an_assignment() {
    let (out, _err, code) = run("\\X=1; builtin echo hi");
    assert_eq!(out, "hi\n");
    assert_eq!(code, 0);
    // The assignment form with a quoted name is still an assignment.
    let (out, _err, code) = run("X=a\\ b; builtin echo [$X]");
    assert_eq!(code, 0);
    assert_eq!(out, "[a b]\n");
}
