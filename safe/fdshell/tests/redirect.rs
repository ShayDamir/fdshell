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

fn temp_path(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!("redirect_{tag}_{}.txt", std::process::id()));
    path.to_str().unwrap().to_string()
}

#[test]
fn space_separated_write_redirect() {
    let path = temp_path("write");
    let (out, err, code) = run(&format!("echo hi > {path}; cat {path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    // `echo`'s output goes to the file; only `cat` writes to stdout.
    assert_eq!(out, "hi\n");
}

#[test]
fn space_separated_read_redirect() {
    let path = temp_path("read");
    std::fs::write(&path, b"data\n").unwrap();
    let (out, err, code) = run(&format!("cat < {path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "data\n");
}

#[test]
fn space_separated_named_fd_redirect() {
    let path = temp_path("stderr");
    // `2> {path}` (the form under test) rehomes fd 2 onto the file; `echo e
    // >&2` then writes through it.
    let (out, err, code) = run(&format!("exec 2> {path}; echo e >&2"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "e\n");
    assert!(out.is_empty());
}

#[test]
fn space_separated_append_redirect() {
    let path = temp_path("append");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!("echo a >> {path}; echo b >> {path}; cat {path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn space_separated_redirect_quoted_path() {
    let path = std::env::temp_dir().join(format!("redirect_quoted_{}.txt", std::process::id()));
    let quoted = path.to_str().unwrap();
    let (out, err, code) = run(&format!("echo hi > \"{quoted}\"; cat \"{quoted}\""));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn space_separated_redirect_after_exec() {
    let path = temp_path("exec");
    let (out, err, code) = run(&format!("exec > {path}; echo hi"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "hi\n");
    assert!(out.is_empty());
}

#[test]
fn space_separated_redirect_in_pipeline() {
    let path = temp_path("pipeline");
    let (out, err, code) = run(&format!("echo a > {path} | cat"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "a\n");
    assert!(out.is_empty());
}

#[test]
fn space_separated_redirect_with_heredoc() {
    let path = temp_path("heredoc");
    let (_out, err, code) = run(&format!("cat <<EOF > {path}\nbody\nEOF"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "body\n");
}

#[test]
fn space_separated_redirect_without_operand_is_error() {
    let (_out, err, code) = run("true >");
    assert_eq!(code, 1);
    assert!(err.contains("invalid redirect"), "stderr={err:?}");
}

#[test]
fn space_separated_redirect_before_semicolon_is_error() {
    let (_out, err, code) = run("echo hi > ;");
    assert_eq!(code, 1);
    assert!(err.contains("invalid redirect"), "stderr={err:?}");
}

// POSIX #2.5: several redirections to one fd are all applied, in the order
// written, and the last one wins. Every test below measures bash 5.3.9.

/// Writes the two paths and returns them as `path1, path2`.
fn two_paths(tag: &str) -> (String, String) {
    (
        temp_path(&format!("{tag}_1")),
        temp_path(&format!("{tag}_2")),
    )
}

#[test]
fn duplicate_write_redirect_last_wins_and_first_is_created() {
    let (p1, p2) = two_paths("dup_write");
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    let (out, err, code) = run(&format!("echo hi >{p1} >{p2}; cat {p2}"));
    // The first target is opened+truncated by its own redirect, so it exists
    // and is empty: a last-wins design that drops the earlier entry would
    // leave `p1` absent, which bash does not do.
    let first = std::fs::read_to_string(&p1).unwrap();
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
    assert!(
        first.is_empty(),
        "first target is created empty, got {first:?}"
    );
}

#[test]
fn duplicate_write_redirect_last_wins_space_separated() {
    // The same rule through the space-separated operator form.
    let (p1, p2) = two_paths("dup_write_sp");
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    let (out, err, code) = run(&format!("echo hi > {p1} > {p2}; cat {p2}"));
    let first = std::fs::read_to_string(&p1).unwrap();
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
    assert!(
        first.is_empty(),
        "first target is created empty, got {first:?}"
    );
}

#[test]
fn duplicate_read_redirect_last_wins() {
    let (p1, p2) = two_paths("dup_read");
    std::fs::write(&p1, b"A\n").unwrap();
    std::fs::write(&p2, b"B\n").unwrap();
    let (out, err, code) = run(&format!("cat <{p1} <{p2}"));
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "B\n");
}

#[test]
fn duplicate_read_first_target_missing_is_error() {
    // Every redirect is opened, so a missing earlier target aborts the command
    // (bash: rc 1, `missing: No such file or directory`, empty stdout).
    let (p1, p2) = two_paths("dup_read_missing");
    std::fs::write(&p2, b"B\n").unwrap();
    let (out, err, code) = run(&format!("cat <{p1} <{p2}"));
    let _ = std::fs::remove_file(&p2);
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    assert!(err.contains("redirect file open failed"), "stderr={err:?}");
}

#[test]
fn duplicate_read_last_target_missing_is_error() {
    // The failure rule is order-independent: a later missing target also aborts.
    let (p1, p2) = two_paths("dup_read_missing2");
    std::fs::write(&p1, b"A\n").unwrap();
    let (out, _err, code) = run(&format!("cat <{p1} <{p2}"));
    let _ = std::fs::remove_file(&p1);
    assert_eq!(code, 1);
    assert!(out.is_empty(), "stdout={out:?}");
}

#[test]
fn duplicate_append_targets_last_wins() {
    let (p1, p2) = two_paths("dup_append");
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    let (out, err, code) = run(&format!("echo a >>{p1} >>{p2}; cat {p2}"));
    let first = std::fs::read_to_string(&p1).unwrap();
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\n");
    assert!(
        first.is_empty(),
        "first target is created empty, got {first:?}"
    );
}

#[test]
fn close_then_reopen_stdin() {
    // `0>&-` then `<file`: the reopen wins, so `cat` prints the file.
    let path = temp_path("close_reopen");
    std::fs::write(&path, b"A\n").unwrap();
    let (out, _err, code) = run(&format!("cat 0>&- <{path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0);
    assert!(out.contains("A"), "stdout={out:?}");
}

#[test]
fn reopen_then_close_stdin() {
    // `cat <file 0>&-`: bash closes the reopened stdin, so `cat` reads nothing
    // and exits 1 (its `Bad file descriptor` lines are `cat`'s own noise).
    let path = temp_path("reopen_close");
    std::fs::write(&path, b"A\n").unwrap();
    let (out, _err, code) = run(&format!("cat <{path} 0>&-"));
    let _ = std::fs::remove_file(&path);
    assert!(out.is_empty(), "stdout={out:?}");
    assert_eq!(code, 1);
}

#[test]
fn exec_duplicate_write_last_wins() {
    // The `exec` path applies redirects in the shell process, in list order:
    // the shell's stdout lands on `p2`, so `p1` is created empty and `p2`
    // holds the later output.
    let (p1, p2) = two_paths("exec_dup");
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    let (_out, err, code) = run(&format!("exec >{p1} >{p2}; echo hi"));
    let first = std::fs::read_to_string(&p1).unwrap();
    let second = std::fs::read_to_string(&p2).unwrap();
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        first.is_empty(),
        "first target is created empty, got {first:?}"
    );
    assert_eq!(second, "hi\n");
}

#[test]
fn duplicate_write_in_pipeline_stage() {
    // The stage's own stdout redirect overrides the pipe write end, which is
    // applied first: `q` gets the text and the pipe carries nothing.
    let (p1, p2) = two_paths("pipe_dup");
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    let (out, err, code) = run(&format!("echo a >{p1} >{p2} | cat"));
    let first = std::fs::read_to_string(&p1).unwrap();
    let second = std::fs::read_to_string(&p2).unwrap();
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    assert!(
        first.is_empty(),
        "first target is created empty, got {first:?}"
    );
    assert_eq!(second, "a\n");
}

#[test]
fn noclobber_duplicate_write_first_target_blocks() {
    // A failing earlier redirection aborts the command before the later one
    // opens: `y` is absent. Reversing the order creates `y` empty and the
    // command still fails on `x`.
    let (x, y) = two_paths("noclobber");
    std::fs::write(&x, b"old").unwrap();
    let _ = std::fs::remove_file(&y);
    let (_out, err, code) = run(&format!("set -o noclobber; echo hi >{x} >{y}"));
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("noclobber"), "stderr={err:?}");
    assert!(
        !std::path::Path::new(&y).exists(),
        "{y} must not be created"
    );

    let (out2, _err2, code2) = run(&format!("set -o noclobber; echo hi >{y} >{x}; cat {y}"));
    assert_eq!(code2, 1);
    assert_eq!(out2, "");
    let created = std::fs::read_to_string(&y).unwrap();
    assert!(
        created.is_empty(),
        "first target is created empty, got {created:?}"
    );
    let _ = std::fs::remove_file(&x);
    let _ = std::fs::remove_file(&y);
}

#[test]
fn brace_expanded_target_is_last_wins() {
    // fdshell brace-expands a redirect target, so `>{a,b}` is two redirects to
    // fd 1 and the last wins. bash reports `ambiguous redirect` (rc 1) because
    // it does not expand a redirect target - a documented divergence.
    let (p1, p2) = two_paths("brace_dup");
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    let (out, err, code) = run(&format!("echo hi >{{{p1},{p2}}}"));
    let first = std::fs::read_to_string(&p1).unwrap();
    let second = std::fs::read_to_string(&p2).unwrap();
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    assert!(
        first.is_empty(),
        "first target is created empty, got {first:?}"
    );
    assert_eq!(second, "hi\n");
}

#[test]
fn heredoc_then_file_stdin_redirect_wins() {
    // `cat <<EOF <file` applies the file after the body, so the file wins.
    let path = temp_path("heredoc_file");
    std::fs::write(&path, b"F1\n").unwrap();
    let (out, err, code) = run(&format!("cat <<EOF <{path}\nbody\nEOF"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "F1\n");
}

#[test]
fn file_stdin_redirect_then_heredoc_wins() {
    // The reverse order: the heredoc is last, so the body wins.
    let path = temp_path("file_heredoc");
    std::fs::write(&path, b"F1\n").unwrap();
    let (out, err, code) = run(&format!("cat <{path} <<EOF\nbody\nEOF"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "body\n");
}
