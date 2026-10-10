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
    // The reverse order: the heredoc is last, so the file wins.
    let path = temp_path("file_heredoc");
    std::fs::write(&path, b"F1\n").unwrap();
    let (out, err, code) = run(&format!("cat <{path} <<EOF\nbody\nEOF"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "body\n");
}

// POSIX #2.2 `>|` (clobber). bash 5.3.9 measures the attached and the
// space-separated form identically: `echo hi >|f` and `echo hi >| f` both write
// `hi\n` and exit 0, with noclobber set.
#[test]
fn clobber_attached_and_bare_forms_write_the_same_file() {
    let a = temp_path("clobber_attached");
    let b = temp_path("clobber_bare");
    let (out, err, code) = run(&format!(
        "set -o noclobber; echo hi >|{a}; echo b >| {b}; cat {a} {b}"
    ));
    let _ = std::fs::remove_file(&a);
    let _ = std::fs::remove_file(&b);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\nb\n", "stdout={out:?}");
}

// The losers' side effects (LESSONS: test the losers, not only the winner):
// `echo NEW >|x >|y` opens and truncates `x`, and `y` (the last to fd 1) gets
// the output. bash 5.3.9: rc 0, `x` empty, `y` = `NEW`.
#[test]
fn clobber_duplicate_last_wins_and_the_loser_is_truncated() {
    let (x, y) = two_paths("clobber_last_wins");
    std::fs::write(&x, b"old\n").unwrap();
    std::fs::write(&y, b"old2\n").unwrap();
    let (_out, err, code) = run(&format!("set -o noclobber; echo NEW >|{x} >|{y}"));
    let first = std::fs::read_to_string(&x).unwrap();
    let second = std::fs::read_to_string(&y).unwrap();
    let _ = std::fs::remove_file(&x);
    let _ = std::fs::remove_file(&y);
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(first.is_empty(), "loser target is truncated, got {first:?}");
    assert_eq!(second, "NEW\n");
}

// A `>` that noclobber blocks is the failing first redirection, so the command
// aborts before the `>|` opens: both files stay untouched. bash 5.3.9: rc 1,
// `x` = `old`, `y` = `old2`.
#[test]
fn clobber_after_a_blocked_target_does_not_run() {
    let (x, y) = two_paths("clobber_blocked_first");
    std::fs::write(&x, b"old\n").unwrap();
    std::fs::write(&y, b"old2\n").unwrap();
    let (out, err, code) = run(&format!("set -o noclobber; echo NEW >{x} >|{y}"));
    let first = std::fs::read_to_string(&x).unwrap();
    let second = std::fs::read_to_string(&y).unwrap();
    let _ = std::fs::remove_file(&x);
    let _ = std::fs::remove_file(&y);
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("noclobber"), "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    assert_eq!(first, "old\n");
    assert_eq!(second, "old2\n");
}

// `>|x >>y` on one command: both targets take effect, so the clobber target is
// truncated and the append target keeps its content plus the new line.
// bash 5.3.9: rc 0, `x` empty, `y` = `old2\nNEW`.
#[test]
fn clobber_and_append_targets_both_take_effect() {
    let (x, y) = two_paths("clobber_append");
    std::fs::write(&x, b"old\n").unwrap();
    std::fs::write(&y, b"old2\n").unwrap();
    let (_out, err, code) = run(&format!("set -o noclobber; echo NEW >|{x} >>{y}"));
    let first = std::fs::read_to_string(&x).unwrap();
    let second = std::fs::read_to_string(&y).unwrap();
    let _ = std::fs::remove_file(&x);
    let _ = std::fs::remove_file(&y);
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        first.is_empty(),
        "clobber target is truncated, got {first:?}"
    );
    assert_eq!(second, "old2\nNEW\n");
}

// A clobber redirect inside a pipeline stage: the stage's stdout goes to the
// file, so the pipe carries nothing. bash 5.3.9: `echo hi >|f | wc -l` prints 0.
#[test]
fn clobber_in_a_pipeline_stage_writes_the_file() {
    let path = temp_path("clobber_pipeline");
    std::fs::write(&path, b"old\n").unwrap();
    let (out, err, code) = run(&format!("set -o noclobber; echo hi >|{path} | wc -l"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "0\n", "stdout={out:?}");
    assert_eq!(body, "hi\n");
}

// Every form without a usable operand is a parse error (fdshell rc 1 with
// `invalid redirect syntax`; bash rc 2 with a syntax error). `>|` shares the
// bare operator's operand rules, so `> | f`, `>||f`, `>>|f`, `<|f` and `&>|f`
// are all rejected, and `>|` at end of input has no operand. Each script
// carries its own target, and the target is never created.
#[test]
fn clobber_forms_without_an_operand_are_parse_errors() {
    let path = temp_path("clobber_reject");
    let _ = std::fs::remove_file(&path);
    for script in [
        "echo hi >|",
        &format!("echo hi >| ; {path}"),
        &format!("echo hi >| | {path}"),
        &format!("echo hi >| >| {path}"),
        &format!("echo hi > | {path}"),
        &format!("echo hi >||{path}"),
        &format!("echo hi >>|{path}"),
        &format!("echo hi <|{path}"),
        &format!("true &>|{path}"),
    ] {
        let (out, err, code) = run(script);
        assert_ne!(code, 0, "script={script:?} stdout={out:?}");
        assert!(
            err.contains("invalid redirect"),
            "script={script:?} stderr={err:?}"
        );
        assert!(
            !std::path::Path::new(&path).exists(),
            "script={script:?} must not create the target"
        );
    }
}

// `cat >| <<EOF`: the bare `>|` has no operand, so the command is a parse
// error. The byte-level `<<` scan and the token-level count agree on the line
// (one operator), so the heredoc body is read as a body region and the failure
// is the redirect, never the `missing terminating delimiter` message.
// bash 5.3.9: rc 2, syntax error at `<<`.
#[test]
fn clobber_before_a_heredoc_is_a_redirect_error_not_a_delimiter_error() {
    let (out, err, code) = run("cat >| <<EOF\nbody\nEOF");
    assert_ne!(code, 0, "stdout={out:?}");
    assert!(err.contains("invalid redirect"), "stderr={err:?}");
    assert!(
        !err.contains("missing terminating delimiter"),
        "stderr={err:?}"
    );
}

// `cmd &>|f` is rejected (bash: syntax error rc 2) and creates no file; the
// `&>|&pid` pidvar form stays a valid background form (`wait $pid` finds it).
#[test]
fn background_clobber_is_rejected_and_the_pidvar_form_stays() {
    let path = temp_path("bg_clobber");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!("true &>|{path}"));
    assert_ne!(code, 0, "stdout={out:?} stderr={err:?}");
    assert!(
        !std::path::Path::new(&path).exists(),
        "{path} must not be created"
    );

    let (out2, err2, code2) = run("true a b &>|&mypid; wait $mypid; builtin echo rc=$?");
    assert_eq!(code2, 0, "stderr={err2:?}");
    assert_eq!(out2, "rc=0\n", "stdout={out2:?}");
}

// Accepted deviation: fdshell never breaks a word at an operator byte (as for
// `a>b`), so `a>|b` is one argument, no file is created. bash 5.3.9 breaks the
// word at `>` and clobbers `b` with `x a`.
#[test]
fn clobber_inside_a_word_stays_one_argument() {
    let path = temp_path("clobber_in_word");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!("echo x a>|{path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, format!("x a>|{path}\n"), "stdout={out:?}");
    assert!(
        !std::path::Path::new(&path).exists(),
        "fdshell must not create the file bash clobbers"
    );
}

// `>|` reaches every operator position: the fd prefix (`2>|` lands on stderr),
// the `/dev/fd/N` dup form, and the fd-var form. Measured against bash 5.3.9,
// which gives the same stdout for each.
#[test]
fn clobber_reaches_the_fd_prefix_fd_path_and_fd_var_forms() {
    let err_path = temp_path("clobber_fd2");
    std::fs::write(&err_path, b"old\n").unwrap();
    let (out, err, code) = run(&format!(
        "set -o noclobber; exec 2>|{err_path}; echo e >&2; echo out; cat {err_path}"
    ));
    let _ = std::fs::remove_file(&err_path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "out\ne\n", "stdout={out:?}");

    // The dup form: `>| /dev/fd/4` copies the already-open fd 4 onto stdout.
    let out_path = temp_path("clobber_fd_path");
    let _ = std::fs::remove_file(&out_path);
    let (out2, err2, code2) = run(&format!(
        "set -o noclobber; exec 4> {out_path}; echo hi >| /dev/fd/4; cat {out_path}"
    ));
    let body2 = std::fs::read_to_string(&out_path).unwrap();
    let _ = std::fs::remove_file(&out_path);
    assert_eq!(code2, 0, "stderr={err2:?}");
    assert_eq!(out2, "hi\n", "stdout={out2:?}");
    assert_eq!(body2, "hi\n");

    // The fd-var form: the var's fd receives the output.
    let var_path = temp_path("clobber_fd_var");
    std::fs::write(&var_path, b"").unwrap();
    let (out3, err3, code3) = run(&format!(
        "set -o noclobber; builtin openat2 --flags O_RDWR {var_path} %>%f; echo hi >|%f; cat {var_path}"
    ));
    let _ = std::fs::remove_file(&var_path);
    assert_eq!(code3, 0, "stderr={err3:?}");
    assert_eq!(out3, "hi\n", "stdout={out3:?}");
}

// A quoted clobber target is one path, mask-protected, exactly as `> "path"`.
#[test]
fn clobber_quoted_path_is_one_target() {
    let path = std::env::temp_dir().join(format!(
        "redirect_clobber_quoted_{}.txt",
        std::process::id()
    ));
    let quoted = path.to_str().unwrap();
    std::fs::write(&path, b"old\n").unwrap();
    let (out, err, code) = run(&format!(
        "set -o noclobber; echo hi >| \"{quoted}\"; cat \"{quoted}\""
    ));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n", "stdout={out:?}");
}
