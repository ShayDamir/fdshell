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

#[test]
fn star_lists_directory_larger_than_one_getdents_buffer() {
    // 200 entries ≈ 6.4 KiB of records, so the glob walk needs more than one
    // 4 KiB getdents64 pass.
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-glob-big-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..200 {
        std::fs::write(dir.join(format!("f{i:03}")), b"x").unwrap();
    }
    let (out, _err, code) = run(dir.to_str().unwrap(), "echo *");
    assert_eq!(code, 0);
    let names: Vec<String> = (0..200).map(|i| format!("f{i:03}")).collect();
    assert_eq!(out, format!("{}\n", names.join(" ")));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Write an executable POSIX script so command-name globbing can exec it.
fn write_script(dir: &str, name: &str, body: &str) {
    let path = std::path::Path::new(dir).join(name);
    std::fs::write(&path, body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(&path).unwrap().permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&path, perm).unwrap();
}

#[test]
fn dotglob_lists_dotfiles() {
    let dir = scratch("dotglob");
    // `*` with dotglob includes the dotfile but never `.`/`..`.
    let (out, _err, code) = run(&dir, "shopt -s dotglob; echo *");
    assert_eq!(code, 0);
    assert_eq!(out, ".hidden a1 a2 b1 file sub x\n");
    // `.*` (a component whose first byte is a literal `.`) lists `.`/`..` too.
    let (out, _err, code) = run(&dir, "echo .*");
    assert_eq!(code, 0);
    assert_eq!(out, ". .. .hidden\n");
    // Without dotglob, `*` skips the dotfile.
    let (out, _err, code) = run(&dir, "echo *");
    assert_eq!(code, 0);
    assert_eq!(out, "a1 a2 b1 file sub x\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failglob_errors_on_no_match() {
    let dir = scratch("failglob");
    let (out, err, code) = run(&dir, "shopt -s failglob; echo zzz*");
    assert_ne!(code, 0);
    assert!(err.contains("no match"), "stderr: {err}");
    assert_eq!(out, "");
    // failglob beats nullglob: the pattern is not dropped, it errors.
    let (out, err, code) = run(&dir, "shopt -s nullglob; shopt -s failglob; echo zzz*");
    assert_ne!(code, 0);
    assert!(err.contains("no match"), "stderr: {err}");
    assert_eq!(out, "");
    // A matching pattern is unaffected.
    let (out, _err, code) = run(&dir, "shopt -s failglob; echo a*");
    assert_eq!(code, 0);
    assert_eq!(out, "a1 a2\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn posix_class_matches() {
    let dir = scratch("class");
    let (out, _err, code) = run(&dir, "echo [[:alpha:]]*");
    assert_eq!(code, 0);
    assert_eq!(out, "a1 a2 b1 file sub x\n");
    // No file starts with a digit: the pattern stays verbatim.
    let (out, _err, code) = run(&dir, "echo [[:digit:]]*");
    assert_eq!(code, 0);
    assert_eq!(out, "[[:digit:]]*\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn redirect_target_globs() {
    let dir = scratch("redir");
    // Two `a*` matches: ambiguous redirect, nothing written.
    let (_out, err, code) = run(&dir, "echo hi >a*");
    assert_ne!(code, 0);
    assert!(err.contains("ambiguous redirect"), "stderr: {err}");
    // A single `b*` match: writes that file.
    let (_out, _err, code) = run(&dir, "echo hi >b*");
    assert_eq!(code, 0);
    assert_eq!(
        std::fs::read_to_string(dir.as_str().to_owned() + "/b1").unwrap(),
        "hi\n"
    );
    // No match: the literal word is created.
    let (_out, _err, code) = run(&dir, "echo hi >zzz*");
    assert_eq!(code, 0);
    assert_eq!(
        std::fs::read_to_string(dir.as_str().to_owned() + "/zzz*").unwrap(),
        "hi\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn command_name_globs() {
    let dir = scratch("cmdname");
    write_script(&dir, "run.sh", "#!/bin/sh\necho ran\n");
    // Sole match: the match is the command.
    let (out, _err, code) = run(&dir, "./run.s*");
    assert_eq!(code, 0);
    assert_eq!(out, "ran\n");
    // Multi-match: the first is the command, the rest are leading args.
    write_script(&dir, "c1.sh", "#!/bin/sh\necho got:$1\n");
    write_script(&dir, "c2.sh", "#!/bin/sh\necho never\n");
    let (out, _err, code) = run(&dir, "./c*.sh");
    assert_eq!(code, 0);
    assert_eq!(out, "got:./c2.sh\n");
    let _ = std::fs::remove_dir_all(&dir);
}
