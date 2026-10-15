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
fn here_string_feeds_stdin() {
    let (out, _err, code) = run("cat <<<hello");
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
}

#[test]
fn here_string_quoted_keeps_spaces() {
    let (out, _err, code) = run("cat <<<\"a b\"");
    assert_eq!(code, 0);
    assert_eq!(out, "a b\n");
}

#[test]
fn here_string_expands_variables() {
    let (out, _err, code) = run("X=hello; cat <<<$X");
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
}

#[test]
fn here_string_bare_form_takes_next_word() {
    let (out, _err, code) = run("cat <<< word");
    assert_eq!(code, 0);
    assert_eq!(out, "word\n");
}

#[test]
fn here_string_appends_trailing_newline() {
    let (out, _err, code) = run("wc -c <<<x");
    assert_eq!(code, 0);
    assert_eq!(out, "2\n");
}

#[test]
fn here_string_counts_lines() {
    let (out, _err, code) = run("wc -l <<<x");
    assert_eq!(code, 0);
    assert_eq!(out, "1\n");
}

#[test]
fn here_string_leaves_other_args_intact() {
    let (out, _err, code) = run("echo a <<<x b");
    assert_eq!(code, 0);
    assert_eq!(out, "a b\n");
}

#[test]
fn here_string_empty_word_is_one_newline() {
    let (out, _err, code) = run("cat <<<\"\"");
    assert_eq!(code, 0);
    assert_eq!(out, "\n");
}

#[test]
fn here_string_empty_quoted_word_keeps_later_args() {
    // `""` is the here-string word, not a no-op: `b` stays an argument
    // (bash: `echo a <<<"" b` → `a b`).
    let (out, _err, code) = run("echo a <<<\"\" b");
    assert_eq!(code, 0);
    assert_eq!(out, "a b\n");
}

#[test]
fn here_string_with_builtin_command() {
    // printf does not read stdin, but the redirect must apply without error.
    let (out, _err, code) = run("printf \"%s\" a <<<hi");
    assert_eq!(code, 0);
    assert_eq!(out, "a");
}

#[test]
fn here_string_expansion_failure_is_reported() {
    // Out-of-range positional index makes the word expansion fail.
    let (_out, err, code) = run("cat <<<$99999999999999999999");
    assert_ne!(code, 0);
    assert!(err.contains("here-string"), "stderr={err:?}");
}

// --- the word's own quote mask (task #269) ----------------------------------
//
// The here-string word is substituted with its own mask, so a `\<char>` pair
// written inside double quotes keeps its backslash (POSIX 2.2.2) and a quoted
// `~` stays literal. Every expectation is measured against bash 5.3.9.

#[test]
fn here_string_quoted_escape_pair_keeps_the_backslash() {
    // bash: `a\ b`; fdshell with the empty mask printed `a b`.
    let (out, _err, code) = run("cat <<<\"a\\ b\"");
    assert_eq!((out.as_str(), code), ("a\\ b\n", 0));
    // The bare form takes the next token's mask verbatim.
    let (out, _err, code) = run("cat <<< \"a\\ b\"");
    assert_eq!((out.as_str(), code), ("a\\ b\n", 0));
    // Single quotes are mask-independent: the pair is data.
    let (out, _err, code) = run("cat <<<'a\\ b'");
    assert_eq!((out.as_str(), code), ("a\\ b\n", 0));
}

#[test]
fn here_string_quoted_dollar_and_backslash_pairs_drop_the_backslash() {
    // The three quoted pairs of POSIX 2.2.2, as everywhere else in fdshell.
    let (out, _err, code) = run("cat <<<\"a\\$b\"");
    assert_eq!((out.as_str(), code), ("a$b\n", 0));
    let (out, _err, code) = run("cat <<<\"a\\\\b\"");
    assert_eq!((out.as_str(), code), ("a\\b\n", 0));
    let (out, _err, code) = run("cat <<<\"a\\`b\"");
    assert_eq!((out.as_str(), code), ("a`b\n", 0));
    // An escaped quote is data, the tokenizer resolves it (bash agrees).
    let (out, _err, code) = run("cat <<<\"a\\\"b\"");
    assert_eq!((out.as_str(), code), ("a\"b\n", 0));
}

#[test]
fn here_string_unquoted_escape_pair_folds() {
    // An unquoted escape pair stays in the token text and folds (POSIX 2.2.1),
    // and the escaped space keeps the word whole — the same word bash builds.
    let (out, _err, code) = run("cat <<<a\\ b");
    assert_eq!((out.as_str(), code), ("a b\n", 0));
    // A trailing `\` of the word is kept.
    let (out, _err, code) = run("cat <<<abc\\");
    assert_eq!((out.as_str(), code), ("abc\\\n", 0));
}

#[test]
fn here_string_quoted_word_never_splits() {
    // The mask is the splitting mask too: the quoted space is one word.
    let (out, _err, code) = run("wc -c <<<\"a b\"");
    assert_eq!((out.as_str(), code), ("4\n", 0));
    let (out, _err, code) = run("wc -c <<<\"a\\ b\"");
    assert_eq!((out.as_str(), code), ("5\n", 0));
}

#[test]
fn here_string_tilde_follows_its_mask() {
    // Unquoted `~` expands to `$HOME`; a quoted `~` is a literal byte (bash
    // keeps it, task #269's mask gate in `substitute/tilde.rs`).
    let home = std::env::var("HOME").unwrap();
    let (out, _err, code) = run("cat <<<~");
    assert_eq!((out.as_str(), code), (format!("{home}\n").as_str(), 0));
    let (out, _err, code) = run("cat <<<\"~\"");
    assert_eq!((out.as_str(), code), ("~\n", 0));
    let (out, _err, code) = run("cat <<<\"~/x\"");
    assert_eq!((out.as_str(), code), ("~/x\n", 0));
    let (out, _err, code) = run("cat <<<~/x");
    assert_eq!(
        (out.as_str(), code),
        (format!("{home}/x\n").as_str(), 0),
        "bash prints {home}/x"
    );
}
