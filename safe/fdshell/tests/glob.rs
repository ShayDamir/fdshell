#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Run a script with the child's cwd set to `cwd` so relative patterns hit
/// the scratch dir (the test harness itself runs from the repo root).
fn run(cwd: &str, script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .current_dir(cwd)
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

/// Scratch dir with the plan's layout: `a1 a2 b1 x .hidden file` and
/// `sub/{alpha,beta,gamma}`; each test gets its own dir (pid + counter).
fn scratch(tag: &str) -> String {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-glob-{tag}-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["a1", "a2", "b1", "x", ".hidden", "file"] {
        std::fs::write(dir.join(name), name.as_bytes()).unwrap();
    }
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    for name in ["alpha", "beta", "gamma"] {
        std::fs::write(sub.join(name), name.as_bytes()).unwrap();
    }
    dir.to_str().unwrap().to_string()
}

#[test]
fn star_matches_sorted_and_excludes_dots() {
    let dir = scratch("star");
    let (out, _err, code) = run(&dir, "echo a*");
    assert_eq!(code, 0);
    assert_eq!(out, "a1 a2\n");
    let (out, _err, code) = run(&dir, "echo *");
    assert_eq!(code, 0);
    assert_eq!(out, "a1 a2 b1 file sub x\n");
    let (out, _err, code) = run(&dir, "echo ?");
    assert_eq!(code, 0);
    assert_eq!(out, "x\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bracket_expressions_match() {
    let dir = scratch("bracket");
    let (out, _err, code) = run(&dir, "echo [ab]1");
    assert_eq!(code, 0);
    assert_eq!(out, "a1 b1\n");
    let (out, _err, code) = run(&dir, "echo [!a]1");
    assert_eq!(code, 0);
    assert_eq!(out, "b1\n");
    let (out, _err, code) = run(&dir, "echo [a-z]1");
    assert_eq!(code, 0);
    assert_eq!(out, "a1 b1\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_match_is_verbatim_or_nullglobbed() {
    let dir = scratch("nomatch");
    let (out, _err, code) = run(&dir, "echo zzz*");
    assert_eq!(code, 0);
    assert_eq!(out, "zzz*\n");
    let (out, _err, code) = run(&dir, "shopt -s nullglob; echo zzz*");
    assert_eq!(code, 0);
    assert_eq!(out, "\n");
    // A leading literal dot still matches hidden names (dot rule).
    let (out, _err, code) = run(&dir, "echo .h??den");
    assert_eq!(code, 0);
    assert_eq!(out, ".hidden\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quoted_bytes_are_literal() {
    let dir = scratch("quoted");
    let (out, _err, code) = run(&dir, "echo \"a*\"");
    assert_eq!(code, 0);
    assert_eq!(out, "a*\n");
    let (out, _err, code) = run(&dir, "echo \"a\"1");
    assert_eq!(code, 0);
    assert_eq!(out, "a1\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn escaped_star_is_not_a_pattern() {
    // Documented divergence (README, plan rule 11): fdshell keeps `\X` in the
    // word, so `a\*` is not a pattern and is passed through verbatim. Bash
    // strips the backslash during tokenization and prints `a*` here.
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-glob-esc-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a*"), b"star").unwrap();
    let (out, _err, code) = run(dir.to_str().unwrap(), "echo a\\*");
    assert_eq!(code, 0);
    assert_eq!(out, "a\\*\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn subdirectory_patterns() {
    let dir = scratch("subdir");
    let (out, _err, code) = run(&dir, "echo sub/*");
    assert_eq!(code, 0);
    assert_eq!(out, "sub/alpha sub/beta sub/gamma\n");
    let (out, _err, code) = run(&dir, "echo */gamma");
    assert_eq!(code, 0);
    assert_eq!(out, "sub/gamma\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trailing_slash_is_directories_only() {
    let dir = scratch("trailing");
    let (out, _err, code) = run(&dir, "echo sub/");
    assert_eq!(code, 0);
    assert_eq!(out, "sub/\n");
    let (out, _err, code) = run(&dir, "echo */");
    assert_eq!(code, 0);
    assert_eq!(out, "sub/\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn symlink_to_dir_is_followed() {
    let dir = scratch("symlink");
    let path = std::path::PathBuf::from(&dir);
    std::os::unix::fs::symlink(path.join("sub"), path.join("link")).unwrap();
    let (out, _err, code) = run(&dir, "echo link/*");
    assert_eq!(code, 0);
    assert_eq!(out, "link/alpha link/beta link/gamma\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn absolute_pattern() {
    let dir = scratch("absolute");
    let (out, _err, code) = run(&dir, &format!("echo {dir}/a*"));
    assert_eq!(code, 0);
    assert_eq!(out, format!("{dir}/a1 {dir}/a2\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn for_list_words_glob() {
    let dir = scratch("forlist");
    let (out, _err, code) = run(&dir, "for f in a*; do echo $f; done");
    assert_eq!(code, 0);
    assert_eq!(out, "a1\na2\n");
    // Quoted for word: one literal iteration; the unquoted `$f` re-globs
    // (bash-compatible).
    let (out, _err, code) = run(&dir, "for f in \"a*\"; do echo $f; done");
    assert_eq!(code, 0);
    assert_eq!(out, "a1 a2\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_words_glob_and_dollar_at_reglobs() {
    let dir = scratch("setwords");
    // `set --` words glob. The plan's `echo $1` line assumes bash's
    // 1-indexed positionals; fdshell's are deliberately 0-indexed ($0 is
    // the first `set --` word, LESSONS.md "Positional parameters are
    // 0-indexed"), so `$#` proves the expansion instead.
    let (out, _err, code) = run(&dir, "set -- b*; echo $#");
    assert_eq!(code, 0);
    assert_eq!(out, "1\n");
    let (out, _err, code) = run(&dir, "set -- b*; echo $@");
    assert_eq!(code, 0);
    assert_eq!(out, "b1\n");
    // Unquoted `$@` re-globs its fields (bash rule, plan rule 12).
    let (out, _err, code) = run(&dir, "set -- \"a*\"; echo $@");
    assert_eq!(code, 0);
    assert_eq!(out, "a1 a2\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pipeline_stage_args_glob() {
    let dir = scratch("pipeline");
    let (out, _err, code) = run(&dir, "echo a* | cat");
    assert_eq!(code, 0);
    assert_eq!(out, "a1 a2\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn builtin_args_glob() {
    let dir = scratch("builtin");
    let (out, _err, code) = run(&dir, r#"builtin printf "%s\n" sub/*"#);
    assert_eq!(code, 0);
    assert_eq!(out, "sub/alpha\nsub/beta\nsub/gamma\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unclosed_bracket_is_literal() {
    let dir = scratch("unclosed");
    let (out, _err, code) = run(&dir, "echo [abc");
    assert_eq!(code, 0);
    assert_eq!(out, "[abc\n");
    let (out, _err, code) = run(&dir, "echo x[abc");
    assert_eq!(code, 0);
    assert_eq!(out, "x[abc\n");
    let _ = std::fs::remove_dir_all(&dir);
}
