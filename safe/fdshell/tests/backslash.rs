#![allow(clippy::unwrap_used)]

use std::os::unix::fs::PermissionsExt;
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

/// A scratch dir of executable files, used as a scratch `PATH` so the folded
/// command name is looked up as a real file (bash runs `a\*` for a file `a*`).
fn scratch_bin(names: &[&str]) -> std::path::PathBuf {
    let dir = scratch(names);
    for name in names {
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\necho ran\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

/// Run in `dir` with `bin_dir` prepended to `PATH`.
fn run_with_path(
    dir: &std::path::Path,
    bin_dir: &std::path::Path,
    script: &str,
) -> (String, String, i32) {
    let path = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap());
    let output = Command::new(BIN)
        .current_dir(dir)
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

fn names_in(dir: &std::path::PathBuf) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    v.sort();
    v
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

/// An escaped separator byte is a word byte: the field count proves the word
/// is one (`set -- a\ b` is one positional), and `a\&&b` prints `a&&b` (the
/// accepted divergence — bash backgrounds `a&`, no job control in fdshell).
#[test]
fn escaped_separators_keep_one_field() {
    let (out, _err, code) = run("set -- a\\ b; builtin echo $#");
    assert_eq!(code, 0);
    assert_eq!(out, "1\n");
    let (out, _err, code) = run("builtin echo a\\&b");
    assert_eq!(code, 0);
    assert_eq!(out, "a&b\n");
    let (out, _err, code) = run("builtin echo a\\&&b");
    assert_eq!(code, 0);
    assert_eq!(out, "a&&b\n");
}

/// An escape pair inside a `case` pattern is a literal member: `f\*o` matches
/// the name `f*o` and never `foobar` (envfilter `glob_match` is `*`-only, so
/// `?`/`[...]` stay out of scope — task #165).
#[test]
fn case_pattern_escape_is_literal() {
    let (out, _err, code) = run("case f*o in f\\*o) builtin echo y;; esac");
    assert_eq!(code, 0);
    assert_eq!(out, "y\n");
    let (out, _err, code) = run("case foobar in f\\*o) builtin echo y;; esac");
    assert_eq!(code, 0);
    assert_eq!(out, "");
}

/// The arith lexers do not fold escape pairs, so `$((1\+2))` is an arithmetic
/// syntax error (as in bash), rc 1.
#[test]
fn escape_pair_in_arith_is_a_syntax_error() {
    let (_out, err, code) = run("builtin echo $((1\\+2))");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(
        err.contains("arithmetic expression has a syntax error"),
        "stderr={err:?}"
    );
}

/// POSIX #4.2: inside double quotes every `\<char>` other than `\$`/`\\` keeps
/// the backslash, and `\"` yields a literal quote.
#[test]
fn in_quote_escape_rules_are_unchanged() {
    let (out, _err, code) = run("builtin echo \"a\\ b\"");
    assert_eq!(code, 0);
    assert_eq!(out, "a\\ b\n");
    let (out, _err, code) = run("builtin echo \"a\\nc\"");
    assert_eq!(code, 0);
    assert_eq!(out, "a\\nc\n");
    let (out, _err, code) = run("X=hi; builtin echo \"a\\$X\"");
    assert_eq!(code, 0);
    assert_eq!(out, "a$X\n");
    let (out, _err, code) = run("builtin echo \"a\\\\b\"");
    assert_eq!(code, 0);
    assert_eq!(out, "a\\b\n");
}

/// An alias body runs the normal tokenizer, so an escape pair after the alias
/// is folded by the same rule: `alias e=echo; e a\*` prints `a*`.
#[test]
fn alias_body_takes_the_same_escape_rule() {
    let (out, _err, code) = run("alias e=echo; e a\\*");
    assert_eq!(code, 0);
    assert_eq!(out, "a*\n");
}

/// Word 0 is folded at parse, so the command name matches the folded text on
/// every lookup surface: builtin dispatch, `command`/`builtin` keywords, `if`
/// body, and `exec` (whose command word is also folded). Bash folds word 0 at
/// tokenization, so `e\cho hi` runs `echo` — fdshell runs the same name.
#[test]
fn command_name_folds_escape_pairs() {
    for script in ["e\\cho hi", "command e\\cho hi", "builtin e\\cho hi"] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script}: stderr={err:?}");
        assert_eq!(out, "hi\n", "{script}");
    }
    let (out, _err, code) = run("if e\\cho hi; then builtin echo t; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "hi\nt\n");
    // `exec` replaces the shell with the folded command word.
    let (out, err, code) = run("exec e\\cho hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
    // `exec builtin` folds the builtin name word (fdshell-only surface: bash
    // has no `builtin` command, so it reports `builtin` not found, rc 127).
    let (out, err, code) = run("exec builtin e\\cho hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
    // With `builtin_first` on, the folded `exec` word is matched against the
    // builtin table before the `PATH` lookup (a fdshell-only option).
    let (out, err, code) = run("set -o builtin_first; exec e\\cho hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
    // The fold reads word 0's own quote mask: a quoted command word keeps its
    // pair, so the literal name `e\cho` is looked up and not found (bash
    // reports `e\cho: command not found` with rc 127; fdshell's missing-command
    // exit is rc 1 — README "Limitations").
    let (_out, err, code) = run("\"e\\cho\" hi");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("e\\cho"), "stderr={err:?}");
}

/// The folded command name is what the glob and the `PATH` lookup see, so an
/// escaped star matches a file literally named `a*`, and an escaped `=` reaches
/// the executable named `X=1` (bash runs it, rc 0).
#[test]
fn folded_command_name_is_looked_up_in_path() {
    let cwd = scratch(&[]);
    let bin = scratch_bin(&["a*", "X=1"]);
    let (out, err, code) = run_with_path(&cwd, &bin, "a\\*");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "ran\n");
    // A `NAME=value`-shaped word with an escaped `=` is a command, not an
    // assignment: the folded `X=1` is the file that runs.
    let (out, err, code) = run_with_path(&cwd, &bin, "\\X=1");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "ran\n");
    let _ = std::fs::remove_dir_all(&cwd);
    let _ = std::fs::remove_dir_all(&bin);
}

/// A redirect target is folded at open time: `> a\*b` writes the file named
/// `a*b`, never a file named `a\*b` (bash folds the target word).
#[test]
fn redirect_target_folds_escape_pairs() {
    let dir = scratch(&[]);
    let (out, err, code) = run_in(&dir, "echo hi > a\\*b");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(names_in(&dir), vec!["a*b".to_string()]);
    assert_eq!(std::fs::read(dir.join("a*b")).unwrap(), b"hi\n");
    // `>>` appends to the same folded name.
    let (out, err, code) = run_in(&dir, "echo yo >> a\\*b");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(names_in(&dir), vec!["a*b".to_string()]);
    assert_eq!(std::fs::read(dir.join("a*b")).unwrap(), b"hi\nyo\n");
    // The read form opens the folded name too (external `cat`).
    let (out, err, code) = run_in(&dir, "cat < a\\*b");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\nyo\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The `for … in` list words are folded before globbing: `a\ b` is one word,
/// and an escaped star is a literal member that globs nothing.
#[test]
fn for_list_words_fold_escape_pairs() {
    let dir = scratch(&["a*", "a1"]);
    let (out, _err, code) = run_in(&dir, "for x in a\\ b; do printf \"[%s]\" \"$x\"; done");
    assert_eq!(code, 0);
    assert_eq!(out, "[a b]");
    let (out, _err, code) = run_in(&dir, "for x in a\\* b; do printf \"[%s]\" \"$x\"; done");
    assert_eq!(code, 0);
    assert_eq!(out, "[a*][b]");
    // An unescaped star in the list still globs (only the pair is folded).
    let (out, _err, code) = run_in(&dir, "for x in a*; do printf \"[%s]\" \"$x\"; done");
    assert_eq!(code, 0);
    assert_eq!(out, "[a*][a1]");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Accepted divergence, pinned: fdshell matches alias names on the raw token
/// text, so `alias a\*=echo` stores the name `a\*` and `a\* hi` finds it.
/// bash folds the name, stores `a*`, and the lookup misses, so it reports
/// `a*: command not found` (rc 127).
#[test]
fn alias_name_keeps_the_escape_pair() {
    let (out, _err, code) = run("alias a\\*=echo; a\\* hi");
    assert_eq!(code, 0);
    assert_eq!(out, "hi\n");
}
