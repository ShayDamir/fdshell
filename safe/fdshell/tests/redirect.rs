#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;
use std::str;
use std::sync::atomic::{AtomicUsize, Ordering};

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

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). The pid-named temp files live inside this dir now. Removal is
/// `.ok()`-tolerant: teardown never panics, and it runs on a failing test's
/// unwind, which is exactly the test that used to leak.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl std::ops::Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<std::path::Path> for Scratch {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

/// A pid + counter scratch dir `fdshell-redirect-<tag>-<pid>-<counter>`;
/// returns the path of a `.txt` file in it plus the guard — bind both.
fn temp_path(tag: &str) -> (String, Scratch) {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fdshell-redirect-{tag}-{}-{}",
        std::process::id(),
        c
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let scratch = Scratch(dir.clone());
    (
        dir.join(format!("{tag}.txt")).to_str().unwrap().to_string(),
        scratch,
    )
}

/// Writes the two paths (one shared scratch dir) and returns them with the
/// guard — bind all three: `let (p1, p2, _sp) = two_paths("tag");`.
fn two_paths(tag: &str) -> (String, String, Scratch) {
    let (p1, scratch) = temp_path(&format!("{tag}_1"));
    let p2 = scratch
        .join(format!("{tag}_2.txt"))
        .to_str()
        .unwrap()
        .to_string();
    (p1, p2, scratch)
}

#[test]
fn space_separated_write_redirect() {
    let (path, _spath) = temp_path("write");
    let (out, err, code) = run(&format!("echo hi > {path}; cat {path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    // `echo`'s output goes to the file; only `cat` writes to stdout.
    assert_eq!(out, "hi\n");
}

#[test]
fn space_separated_read_redirect() {
    let (path, _spath) = temp_path("read");
    std::fs::write(&path, b"data\n").unwrap();
    let (out, err, code) = run(&format!("cat < {path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "data\n");
}

#[test]
fn space_separated_named_fd_redirect() {
    let (path, _spath) = temp_path("stderr");
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
    let (path, _spath) = temp_path("append");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!("echo a >> {path}; echo b >> {path}; cat {path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn space_separated_redirect_quoted_path() {
    let dir = scratch("quoted");
    let path = dir.join("f.txt");
    let quoted = path.to_str().unwrap();
    let (out, err, code) = run(&format!("echo hi > \"{quoted}\"; cat \"{quoted}\""));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn space_separated_redirect_after_exec() {
    let (path, _spath) = temp_path("exec");
    let (out, err, code) = run(&format!("exec > {path}; echo hi"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "hi\n");
    assert!(out.is_empty());
}

#[test]
fn space_separated_redirect_in_pipeline() {
    let (path, _spath) = temp_path("pipeline");
    let (out, err, code) = run(&format!("echo a > {path} | cat"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "a\n");
    assert!(out.is_empty());
}

#[test]
fn space_separated_redirect_with_heredoc() {
    let (path, _spath) = temp_path("heredoc");
    let (_out, err, code) = run(&format!("cat <<EOF > {path}\nbody\nEOF"));
    let body = std::fs::read_to_string(&path).unwrap();
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

#[test]
fn duplicate_write_redirect_last_wins_and_first_is_created() {
    let (p1, p2, _sp) = two_paths("dup_write");
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
    let (p1, p2, _sp) = two_paths("dup_write_sp");
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
    let (p1, p2, _sp) = two_paths("dup_read");
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
    let (p1, p2, _sp) = two_paths("dup_read_missing");
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
    let (p1, p2, _sp) = two_paths("dup_read_missing2");
    std::fs::write(&p1, b"A\n").unwrap();
    let (out, _err, code) = run(&format!("cat <{p1} <{p2}"));
    let _ = std::fs::remove_file(&p1);
    assert_eq!(code, 1);
    assert!(out.is_empty(), "stdout={out:?}");
}

#[test]
fn duplicate_append_targets_last_wins() {
    let (p1, p2, _sp) = two_paths("dup_append");
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    let (out, err, code) = run(&format!("echo a >>{p1} >>{p2}; cat {p2}"));
    let first = std::fs::read_to_string(&p1).unwrap();
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
    let (path, _spath) = temp_path("close_reopen");
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
    let (path, _spath) = temp_path("reopen_close");
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
    let (p1, p2, _sp) = two_paths("exec_dup");
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
    let (p1, p2, _sp) = two_paths("pipe_dup");
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    let (out, err, code) = run(&format!("echo a >{p1} >{p2} | cat"));
    let first = std::fs::read_to_string(&p1).unwrap();
    let second = std::fs::read_to_string(&p2).unwrap();
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
    let (x, y, _sp) = two_paths("noclobber");
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
}

#[test]
fn brace_expanded_target_is_last_wins() {
    // fdshell brace-expands a redirect target, so `>{a,b}` is two redirects to
    // fd 1 and the last wins. bash reports `ambiguous redirect` (rc 1) because
    // it does not expand a redirect target - a documented divergence.
    let (p1, p2, _sp) = two_paths("brace_dup");
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    let (out, err, code) = run(&format!("echo hi >{{{p1},{p2}}}"));
    let first = std::fs::read_to_string(&p1).unwrap();
    let second = std::fs::read_to_string(&p2).unwrap();
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
    let (path, _spath) = temp_path("heredoc_file");
    std::fs::write(&path, b"F1\n").unwrap();
    let (out, err, code) = run(&format!("cat <<EOF <{path}\nbody\nEOF"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "F1\n");
}

#[test]
fn file_stdin_redirect_then_heredoc_wins() {
    // The reverse order: the heredoc is last, so the body wins.
    let (path, _spath) = temp_path("file_heredoc");
    std::fs::write(&path, b"F1\n").unwrap();
    let (out, err, code) = run(&format!("cat <{path} <<EOF\nbody\nEOF"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "body\n");
}

// POSIX #2.2 `>|` (clobber). bash 5.3.9 measures the attached and the
// space-separated form identically: `echo hi >|f` and `echo hi >| f` both write
// `hi\n` and exit 0, with noclobber set.
#[test]
fn clobber_attached_and_bare_forms_write_the_same_file() {
    let (a, _sa) = temp_path("clobber_attached");
    let (b, _sb) = temp_path("clobber_bare");
    let (out, err, code) = run(&format!(
        "set -o noclobber; echo hi >|{a}; echo b >| {b}; cat {a} {b}"
    ));
    let _ = std::fs::remove_file(&b);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\nb\n", "stdout={out:?}");
}

// The losers' side effects (LESSONS: test the losers, not only the winner):
// `echo NEW >|x >|y` opens and truncates `x`, and `y` (the last to fd 1) gets
// the output. bash 5.3.9: rc 0, `x` empty, `y` = `NEW`.
#[test]
fn clobber_duplicate_last_wins_and_the_loser_is_truncated() {
    let (x, y, _sp) = two_paths("clobber_last_wins");
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
    let (x, y, _sp) = two_paths("clobber_blocked_first");
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
    let (x, y, _sp) = two_paths("clobber_append");
    std::fs::write(&x, b"old\n").unwrap();
    std::fs::write(&y, b"old2\n").unwrap();
    let (_out, err, code) = run(&format!("set -o noclobber; echo NEW >|{x} >>{y}"));
    let first = std::fs::read_to_string(&x).unwrap();
    let second = std::fs::read_to_string(&y).unwrap();
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
    let (path, _spath) = temp_path("clobber_pipeline");
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
    let (path, _spath) = temp_path("clobber_reject");
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

// A quoted word is not `>`-terminated, so `cat "a>"| <<EOF` keeps the `|` as
// the pipeline pipe: `<<EOF` is the second stage's command word, the body lines
// are their own commands, and the two `<<` counts agree at 0 (no body region).
// bash 5.3.9: rc 0, `cat: 'a>': No such file`; fdshell exits 1 because `body`
// is not a command, and master measures the same rc 1 with the same stderr.
#[test]
fn quoted_operator_word_keeps_the_pipe_and_the_body_free() {
    let (out, err, code) = run("cat \"a>\"| <<EOF\nbody\nEOF");
    assert_eq!(code, 1, "stdout={out:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    assert!(
        err.contains("\"body\" not found"),
        "the body line must stay a command, stderr={err:?}"
    );
    assert!(
        !err.contains("missing terminating delimiter"),
        "stderr={err:?}"
    );
}

// The agreement at 0 keeps the next heredoc's body where it belongs: a
// divergence shifts the body regions, so `body1` would be eaten by the `<<A`
// span and the script would fail with `missing terminating delimiter line 'A'`
// (measured on the pre-fix tree). bash 5.3.9: rc 0, stdout `hi`; master rc 0.
#[test]
fn quoted_operator_word_does_not_shift_the_next_heredoc_body() {
    let (out, err, code) = run("cat \"a>\"| <<A\nbody1\nA\necho hi <<B\nbody2\nB");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n", "stdout={out:?}");
}

// `cmd &>|f` is rejected (bash: syntax error rc 2) and creates no file; the
// `&>|&pid` pidvar form stays a valid background form (`wait $pid` finds it).
#[test]
fn background_clobber_is_rejected_and_the_pidvar_form_stays() {
    let (path, _spath) = temp_path("bg_clobber");
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
    let (path, _spath) = temp_path("clobber_in_word");
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
    let (err_path, _serrpath) = temp_path("clobber_fd2");
    std::fs::write(&err_path, b"old\n").unwrap();
    let (out, err, code) = run(&format!(
        "set -o noclobber; exec 2>|{err_path}; echo e >&2; echo out; cat {err_path}"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "out\ne\n", "stdout={out:?}");

    // The dup form: `>| /dev/fd/4` copies the already-open fd 4 onto stdout.
    let (out_path, _soutpath) = temp_path("clobber_fd_path");
    let _ = std::fs::remove_file(&out_path);
    let (out2, err2, code2) = run(&format!(
        "set -o noclobber; exec 4> {out_path}; echo hi >| /dev/fd/4; cat {out_path}"
    ));
    let body2 = std::fs::read_to_string(&out_path).unwrap();
    assert_eq!(code2, 0, "stderr={err2:?}");
    assert_eq!(out2, "hi\n", "stdout={out2:?}");
    assert_eq!(body2, "hi\n");

    // The fd-var form: the var's fd receives the output.
    let (var_path, _svarpath) = temp_path("clobber_fd_var");
    std::fs::write(&var_path, b"").unwrap();
    let (out3, err3, code3) = run(&format!(
        "set -o noclobber; builtin openat2 --flags O_RDWR {var_path} %>%f; echo hi >|%f; cat {var_path}"
    ));
    assert_eq!(code3, 0, "stderr={err3:?}");
    assert_eq!(out3, "hi\n", "stdout={out3:?}");
}

// A quoted clobber target is one path, mask-protected, exactly as `> "path"`.
#[test]
fn clobber_quoted_path_is_one_target() {
    let dir = scratch("clobber_quoted");
    let path = dir.join("f.txt");
    let quoted = path.to_str().unwrap();
    std::fs::write(&path, b"old\n").unwrap();
    let (out, err, code) = run(&format!(
        "set -o noclobber; echo hi >| \"{quoted}\"; cat \"{quoted}\""
    ));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n", "stdout={out:?}");
}

// POSIX/bash: the bare `&>` / `&>>` forms take the next word as their operand
// and write fd 1 *and* fd 2 to it (`&> f` is `> f 2> f`). Measured against
// bash 5.3.9; task #145.
#[test]
fn space_separated_combined_redirect() {
    let (path, _spath) = temp_path("combined_write");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!("echo hi &> {path}; cat {path}"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n", "stdout={out:?}");
    assert_eq!(body, "hi\n");
}

#[test]
fn space_separated_combined_redirect_captures_stderr() {
    let (path, _spath) = temp_path("combined_stderr");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!("echo o; echo e 2>&1 &> {path}; cat {path}"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    // Only `echo o` and the final `cat` reach the terminal.
    assert_eq!(out, "o\ne\n", "stdout={out:?}");
    assert_eq!(body, "e\n");
}

#[test]
fn space_separated_combined_append() {
    let (path, _spath) = temp_path("combined_append");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!("echo a &>> {path}; echo b &>> {path}; cat {path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n", "stdout={out:?}");
}

#[test]
fn space_separated_combined_redirect_quoted_operand() {
    let dir = scratch("combined_quoted");
    let path = dir.join("f.txt");
    let quoted = path.to_str().unwrap();
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!("echo hi &> \"{quoted}\"; cat \"{quoted}\""));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n", "stdout={out:?}");
}

#[test]
fn space_separated_combined_redirect_without_operand_is_error() {
    let (_out, err, code) = run("true &>");
    assert_eq!(code, 1);
    assert!(err.contains("invalid redirect"), "stderr={err:?}");
    let (_out, err, code) = run("true &>>");
    assert_eq!(code, 1);
    assert!(err.contains("invalid redirect"), "stderr={err:?}");
}

#[test]
fn space_separated_combined_redirect_before_pipe_is_error() {
    let (path, _spath) = temp_path("combined_before_pipe");
    let _ = std::fs::remove_file(&path);
    let (_out, err, code) = run(&format!("echo hi &> | {path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("invalid redirect"), "stderr={err:?}");
    assert!(
        !std::path::Path::new(&path).exists(),
        "the rejected `|` operand must not create a file"
    );
}

#[test]
fn space_separated_combined_redirect_var_operand_is_error() {
    // `&>%x` (attached) is fdshell's fd-variable redirect; the spaced operand
    // is the plain-word rule's rejection, exactly as `> %x`.
    let (_out, err, code) = run("echo hi &> %x");
    assert_eq!(code, 1);
    assert!(err.contains("invalid redirect"), "stderr={err:?}");
}

#[test]
fn space_separated_combined_redirect_fd_path() {
    let (path, _spath) = temp_path("combined_fd_path");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!(
        "exec 4> {path}; echo hi &> /dev/fd/4; exec 4>&-; cat {path}"
    ));
    let body = std::fs::read_to_string(&path).unwrap();
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n", "stdout={out:?}");
    assert_eq!(body, "hi\n");
}

// --- the `&>` family opens its operand once (task #251) ---------------------
//
// `cmd &> f` is bash's `> f 2>&1`: one `open()`, `dup2`'d onto fd 1 and fd 2,
// so the two streams share one offset and append to each other. Every value
// below is `bash -c` (5.3.9) measured 2026-10-19 in a scratch dir; the old
// two-open behavior wrote `e\n\n` in the first two tests (fd 2's open landed at
// offset 0 and overwrote fd 1's bytes).

#[test]
fn combined_redirect_shares_one_file_description() {
    let (path, _spath) = temp_path("combined_shared");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!(
        "f(){{ echo hi; echo e >&2; }}; f &> {path}; cat {path}"
    ));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "hi\ne\n", "fd 2's write appends after fd 1's");
    assert_eq!(out, "hi\ne\n", "stdout={out:?}");
}

#[test]
fn exec_combined_redirect_shares_one_file_description() {
    // The in-shell (`exec`) scope. The shell's own fds are redirected, so fd 1
    // and fd 2 are saved to fd 3 and restored at the end — the probe's tail
    // `exec 1>&2` would make the `cat` write into the file it reads.
    let (path, _spath) = temp_path("exec_combined_shared");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!(
        "exec 3>&1; exec &> {path}; echo hi; echo e >&2; exec 1>&3; exec 2>&3; exec 3>&-; cat {path}"
    ));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "hi\ne\n", "the scope shares one description too");
    assert_eq!(out, "hi\ne\n", "stdout={out:?}");
}

#[test]
fn combined_append_writes_in_order() {
    let (path, _spath) = temp_path("combined_append_order");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!(
        "f(){{ echo hi; echo e >&2; }}; f &>> {path}; f &>> {path}; cat {path}"
    ));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "hi\ne\nhi\ne\n");
    assert_eq!(out, "hi\ne\nhi\ne\n", "stdout={out:?}");
}

#[test]
fn combined_redirect_fd_path_operand_shares_one_description() {
    // `&> /dev/fd/4` stays a path source (LESSONS), and the sharing rule
    // applies to it: the single open of the magic path lands on the fd the
    // shell's `exec 4>` holds, and fd 1 and fd 2 append to each other.
    let (path, _spath) = temp_path("combined_shared_fd_path");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!(
        "exec 4> {path}; f(){{ echo hi; echo e >&2; }}; f &> /dev/fd/4; exec 4>&-; cat {path}"
    ));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "hi\ne\n");
    assert_eq!(out, "hi\ne\n", "stdout={out:?}");
}

#[test]
fn separate_write_and_stderr_redirects_keep_two_opens() {
    // Two operators, two opens: `> f 2> f` is **not** the `&>` family, and fd 2
    // truncates at offset 0, overwriting fd 1's write. The trailing newline of
    // `hi\n` survives at byte 2 — the exact bytes are pinned so a "share every
    // same-path pair" change cannot pass.
    let (path, _spath) = temp_path("two_opens");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!(
        "f(){{ echo hi; echo e >&2; }}; f > {path} 2> {path}; cat {path}"
    ));
    let body = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, b"e\n\n", "the second open wins at offset 0");
    assert_eq!(out, "e\n\n", "stdout={out:?}");
}

#[test]
fn combined_redirect_noclobber_blocks_existing_file() {
    let (path, _spath) = temp_path("combined_noclobber");
    std::fs::write(&path, b"old\n").unwrap();
    let (_out, err, code) = run(&format!(
        "set -o noclobber; f(){{ echo hi; echo e >&2; }}; f &> {path}"
    ));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(
        err.contains("cannot overwrite existing file"),
        "stderr={err:?}"
    );
    assert_eq!(body, "old\n", "the blocked open leaves the file untouched");
}

#[test]
fn combined_redirect_noclobber_creates_fresh_file() {
    // The two-open form failed here: fd 1's exclusive create made the path
    // exist, and fd 2's second exclusive open then hit `EEXIST` (bash opens
    // once, so `noclobber` is a no-op on a fresh file).
    let (path, _spath) = temp_path("combined_noclobber_fresh");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!(
        "set -o noclobber; f(){{ echo hi; echo e >&2; }}; f &> {path}; cat {path}"
    ));
    let body = std::fs::read_to_string(&path).unwrap();
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "hi\ne\n");
    assert_eq!(out, "hi\ne\n", "stdout={out:?}");
}

// --- the `&>` def's fd-2 half is applied in fd 1's table slot (task #251) ----
//
// Cause: `insert_redirect` keeps a command's redirect list sorted by
// `export_to` ("the last entry for a fd wins"), and the `&>` def carries its
// second target in `extra_to`, which is **not** a table entry. So the def's
// fd-2 half is applied in fd 1's slot — before every def that targets fd 2 —
// and a `2>&-` / `2>&1` written *after* `&>` wins fd 2, where bash applies
// `&>` later and reopens it. These three shapes are the recorded consequences,
// measured on this branch (3/3 runs each) against bash 5.3.9 and master
// `b68878c`, 2026-10-19. Task #337 owns the fix: two *table* entries sharing
// one open by identity, never path equality (`> f 2> f` stays two opens —
// `separate_write_and_stderr_redirects_keep_two_opens`).

#[test]
fn combined_redirect_after_close_stderr_fails() {
    // `f 2>&- &> z`: the later `2>&-` wins fd 2, so fd 2 is closed and the
    // child's own `>&2` has nothing to duplicate. fdshell's launch failure is
    // fatal, so the trailing `echo rc=$?` never runs. bash 5.3.9: rc 0, stdout
    // `rc=0`, `z` = `e\n`; master `b68878c`: rc 0, `z` = `e\n`.
    let (path, _spath) = temp_path("combined_after_close_stderr");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!(
        "f(){{ echo e >&2; }}; f 2>&- &> {path}; echo rc=$?"
    ));
    let body = std::fs::read(&path).unwrap();
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(
        out.is_empty(),
        "the fatal launch aborts the shell, stdout={out:?}"
    );
    assert!(
        err.contains("launch failed") && err.contains("fd 2 is not open"),
        "stderr={err:?}"
    );
    assert!(
        body.is_empty(),
        "the operand is opened before the fd-2 failure, got {body:?}"
    );
}

#[test]
fn combined_append_after_close_stderr_fails() {
    // The same shape through `&>>`: `echo hi` reaches the file, `echo e >&2`
    // fails and the shell aborts. bash 5.3.9: rc 0, stdout `rc=0`, `z` =
    // `hi\ne\n`; master `b68878c`: rc 0, `z` = `hi\ne\n`.
    let (path, _spath) = temp_path("combined_append_after_close_stderr");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!(
        "f(){{ echo hi; echo e >&2; }}; f 2>&- &>> {path}; echo rc=$?"
    ));
    let body = std::fs::read_to_string(&path).unwrap();
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
    assert!(
        err.contains("launch failed") && err.contains("fd 2 is not open"),
        "stderr={err:?}"
    );
    assert_eq!(body, "hi\n", "only fd 1's half of the def survives");
}

#[test]
fn combined_redirect_after_dup_stderr_to_stdout_keeps_pre_redirect_fd1() {
    // `f 2>&1 &> z`: the later `2>&1` wins fd 2 and is resolved against fd 1
    // **before** `&>`'s export, so fd 2 ends as the pre-redirect fd 1 (the
    // terminal) and the `>&2` write escapes there. bash 5.3.9: `z` = `hi\ne\n`,
    // terminal silent; master `b68878c`: `z` = `e\n\n`, terminal silent.
    // (#337's class: resolve-then-export sees the pre-redirect fd 1.)
    let (path, _spath) = temp_path("combined_after_dup_stderr");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!("f(){{ echo hi; echo e >&2; }}; f 2>&1 &> {path}"));
    let body = std::fs::read_to_string(&path).unwrap();
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "e\n", "fd 2 is the pre-redirect fd 1, stdout={out:?}");
    assert!(err.is_empty(), "stderr={err:?}");
    assert_eq!(body, "hi\n", "fd 1 keeps the shared open description");
}

// --- the bare-operand guard is mask-gated (task #323) -----------------------
//
// The operand of a bare `>`/`>>`/`<`/`<>`/`>|`/`&>`/`&>>` is an operator only
// when the leading `<`/`>`/`&`/`%` byte is **unquoted**: POSIX #4.1 removes
// quotes after tokenization, so bash opens the file `&f` for `> "&f"` while
// `> &f` is a syntax error (bash rc 2, fdshell rc 1). Every expectation is
// `bash -c` (5.3.9) and this tree's build, measured 2026-10-18 in a scratch
// dir (literal `&`-leading names; pid + counter per test — LESSONS "tests in
// one binary share the process").
fn scratch(tag: &str) -> Scratch {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell-redirect-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

#[test]
fn bare_write_redirect_quoted_operator_operand_is_a_file() {
    let dir = scratch("qwrite");
    let dir = dir.to_str().unwrap();
    let (out, err, code) = run(&format!("cd {dir}; echo hi > \"&f\"; cat \"&f\""));
    let body = std::fs::read_to_string(std::path::Path::new(dir).join("&f")).unwrap();
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err:?}");
    assert_eq!(body, "hi\n");
}

#[test]
fn bare_read_redirect_quoted_operator_operand_is_a_file() {
    let dir = scratch("qread");
    let dir = dir.to_str().unwrap();
    std::fs::write(std::path::Path::new(dir).join("&f"), b"data\n").unwrap();
    let (out, err, code) = run(&format!("cd {dir}; cat < \"&f\""));
    assert_eq!((out.as_str(), code), ("data\n", 0), "stderr={err:?}");
}

#[test]
fn bare_append_redirect_quoted_operator_operand_appends() {
    let dir = scratch("qappend");
    let dir = dir.to_str().unwrap();
    let (out, err, code) = run(&format!(
        "cd {dir}; echo a >> \"&f\"; echo b >> \"&f\"; cat \"&f\""
    ));
    assert_eq!((out.as_str(), code), ("a\nb\n", 0), "stderr={err:?}");
}

#[test]
fn numbered_readwrite_clobber_quoted_operator_operands_are_files() {
    let dir = scratch("qforms");
    let dir = dir.to_str().unwrap();
    // `3>` and `<>` redirect fd 3 / fd 0 — stdout keeps `hi`, and the operand
    // keeps its content (`<>` opens read-write, writing nothing); `>|` and `>`
    // redirect stdout into it. All four open the `&`-leading file (bash rc 0).
    std::fs::write(std::path::Path::new(dir).join("&f<>"), b"keep\n").unwrap();
    for (op, want_out, want_body) in [
        ("3>", "hi\n", ""),
        ("<>", "hi\nkeep\n", "keep\n"),
        (">|", "hi\n", "hi\n"),
        (">", "hi\n", "hi\n"),
    ] {
        let (out, err, code) = run(&format!(
            "cd {dir}; echo hi {op} \"&f{op}\"; cat \"&f{op}\""
        ));
        let body =
            std::fs::read_to_string(std::path::Path::new(dir).join(format!("&f{op}"))).unwrap();
        assert_eq!((out.as_str(), code), (want_out, 0), "{op}: stderr={err:?}");
        assert_eq!(body, want_body, "{op}");
    }
}

#[test]
fn combined_redirect_quoted_operator_operand_is_a_file() {
    let dir = scratch("qcombined");
    let dir = dir.to_str().unwrap();
    let (out, err, code) = run(&format!("cd {dir}; echo hi &> \"&f\"; cat \"&f\""));
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err:?}");
    let (out, err, code) = run(&format!(
        "cd {dir}; echo a &>> \"&g\"; echo e 2>&1 &>> \"&g\"; cat \"&g\""
    ));
    assert_eq!((out.as_str(), code), ("a\ne\n", 0), "stderr={err:?}");
}

#[test]
fn partially_quoted_operator_operand_is_a_file() {
    // `"&"f`: the quoted leading byte is what the mask gate reads; the word
    // is `&f` and bash creates the file `&f` (rc 0).
    let dir = scratch("qpartial");
    let dir = dir.to_str().unwrap();
    let (out, err, code) = run(&format!("cd {dir}; echo hi > \"&\"f; cat \"&f\""));
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err:?}");
}

#[test]
fn multi_redirect_quoted_operator_operands_match_bash() {
    let dir = scratch("qmulti");
    let dir = dir.to_str().unwrap();
    let (out, err, code) = run(&format!(
        "cd {dir}; echo seed > \"&f\"; cat > \"&f\" < \"&f\""
    ));
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let (out, err, code) = run(&format!("cd {dir}; cat >> \"&f\" << \"Q\"\nbody\nQ"));
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let body = std::fs::read_to_string(std::path::Path::new(dir).join("&f")).unwrap();
    assert_eq!(body, "body\n", "the `>` truncated, the heredoc appended");
    let (out, err, code) = run(&format!("cd {dir}; cat <<< \">x\" 2> \"&g\""));
    assert!(
        std::path::Path::new(dir).join("&g").exists(),
        "the stderr redirect created its file"
    );
    assert_eq!((out.as_str(), code), (">x\n", 0), "stderr={err:?}");
}

#[test]
fn unquoted_operator_operands_stay_syntax_errors() {
    // The unquoted operands keep their current rc: bash gives rc 2 for the
    // `&`/`<`/`>` shapes (fdshell's parse error is rc 1), `> ""&f` is bash's
    // `>` taking `""` then `&` as a control operator (rc 127), and `> %v` /
    // `<<< %v` is fdshell's deliberate `%`-namespace guard arm (POSIX_COMPATIBILITY:
    // the spaced `%var` operand is rejected so it never silently creates a file
    // named `%v`; where bash creates it, rc 0). The quoted empty operand
    // `> ""` stays the guard's `is_empty` arm — owning task #330, bash's
    // `bash: : No such file` is also rc 1.
    for script in [
        "cat > &f",
        "cat > &\"f\"",
        "cat > \"\"&f",
        "cat > %v",
        "cat <<< %v",
        "cat > \"\"",
    ] {
        let dir = scratch("unquoted");
        let dir = dir.to_str().unwrap();
        let (out, err, code) = run(&format!("cd {dir}; {script}"));
        let files: Vec<_> = std::fs::read_dir(dir).unwrap().collect();
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err:?}");
        assert!(err.contains("invalid redirect"), "{script}: stderr={err:?}");
        assert!(
            files.is_empty(),
            "{script}: the rejected operand created a file in {dir:?}"
        );
    }
}

// --- quoted bytes end a redirect operator (tasks #250, #264) -----------------
//
// POSIX XCU 2.9.1 builds a redirection out of **unquoted** bytes: the
// io-number digits and every operator byte. A quote byte ends the operator
// and begins the word, so a fully quoted word — and any word with a quote
// byte in its operator — is a plain argument. The parsers gate on the
// token's quote mask (`redirect::quoted_bytes`, subsuming #250's `fq` flag).
// Every value below is `bash -c` (5.3.9) and this tree's build, measured
// 2026-10-20 in a scratch dir; the scratch listing is the assert that kills
// the redirect arms.

/// Run `script` in `dir` and assert its stdout/rc plus the bash-measured fact
/// that the fully quoted `&>`-family word created **no** file.
fn assert_word_opens_no_file(dir: &str, script: &str, want_out: &str) {
    let (out, err, code) = run(&format!("cd {dir}; {script}"));
    assert_eq!(
        (out.as_str(), code),
        (want_out, 0),
        "{script}: stderr={err:?}"
    );
    let files: Vec<_> = std::fs::read_dir(dir).unwrap().collect();
    assert!(
        files.is_empty(),
        "{script}: the fully quoted word created a file in {dir:?}"
    );
}

#[test]
fn fully_quoted_combined_word_is_an_argument() {
    let dir = scratch("fq_combined");
    let dir = dir.to_str().unwrap();
    for (word, want) in [
        ("'&>q.txt'", "&>q.txt\n"),
        ("\"&>q.txt\"", "&>q.txt\n"),
        ("'&>>q.txt'", "&>>q.txt\n"),
    ] {
        assert_word_opens_no_file(dir, &format!("echo {word}"), want);
    }
}

#[test]
fn fully_quoted_bare_combined_operator_is_an_argument() {
    // The bare arms take the next token as their operand: both words stay
    // arguments, and bash opens nothing.
    let dir = scratch("fq_bare");
    let dir = dir.to_str().unwrap();
    for (script, want) in [
        ("echo hi '&>' f", "hi &> f\n"),
        ("echo hi '&>>' f", "hi &>> f\n"),
    ] {
        assert_word_opens_no_file(dir, script, want);
    }
}

#[test]
fn fully_quoted_pidvar_word_is_an_argument() {
    // The unquoted `&>&p` form *backgrounds* its command, which is what makes
    // the full-stdout assert the discriminator: the ungated tree eats the
    // literal word and prints `tail\nhi\n`, the gated tree keeps the literal
    // and prints it first. Probing the pid variable asserts nothing —
    // measured, it reads empty for a `&>&p` job too.
    let dir = scratch("fq_pidvar");
    let dir = dir.to_str().unwrap();
    for (word, want) in [
        ("'&>&p'", "hi &>&p\ntail\n"),
        ("'&>|&p'", "hi &>|&p\ntail\n"),
    ] {
        let script = format!("echo hi {word}; builtin echo tail");
        assert_word_opens_no_file(dir, &script, want);
    }
}

#[test]
fn fully_quoted_bare_combined_operator_with_a_heredoc_is_an_argument() {
    // The byte-level heredoc scan honors quotes, so the body was already one
    // region; the token layer turned the quoted `&>` into a bare operator that
    // took the next word (`b`) — or the `<<EOF` itself, the second row's
    // `invalid redirect` parse error (rc 1) — as its operand. After the gate
    // the two layers count the same words and the line runs, as bash does.
    let dir = scratch("fq_heredoc");
    let dir = dir.to_str().unwrap();
    for (script, want) in [
        ("echo a '&>' b <<EOF\nbody\nEOF", "a &> b\n"),
        ("echo '&>' <<EOF\nbody\nEOF", "&>\n"),
    ] {
        assert_word_opens_no_file(dir, script, want);
    }
}

#[test]
fn quoted_combined_operator_bytes_are_an_argument() {
    // Task #264 flips the deviation #250 pinned as-is: a quote byte inside
    // the `&>` operator ends the operator (POSIX XCU 2.9.1 reads only
    // unquoted operator bytes), so the word is an argument. `'&>'f` is bash
    // parity: bash prints `&>f` and opens nothing (measured 2026-10-20).
    // `"&">y` is the documented divergence: bash splits the word at the
    // unquoted `>` and writes `y` = `x &`; fdshell never splits a word at an
    // operator byte, so the whole word prints (`POSIX_COMPATIBILITY.md`
    // "Operators are recognized at word starts", the word-split family).
    let dir = scratch("partial_operator_word");
    let dir = dir.to_str().unwrap();
    assert_word_opens_no_file(dir, "echo '&>'f", "&>f\n");
    assert_word_opens_no_file(dir, "echo x \"&\">y", "x &>y\n");
    // The append and var twins: `'&>>'f` and `'&>'%v` print their literal
    // text in bash, and the gate keeps them arguments here too — the `%v`
    // twin no longer dies on the unset fd variable (bash rc 0).
    assert_word_opens_no_file(dir, "echo a '&>>'f", "a &>>f\n");
    assert_word_opens_no_file(dir, "echo a '&>'%v", "a &>%v\n");
}

// --- the partial-quoting family, task #264 -----------------------------------
//
// POSIX XCU 2.9.1: `<redirection> ::= [[io-number] redirection-operator] word`
// over **unquoted** bytes. A quoted first operator byte makes the whole word
// an argument; a quoted *continuation* byte ends the operator there and the
// rest is the operand word; quoted digits are no io-number, so the operator
// takes its default descriptor. Every row is `bash -c` (5.3.9) and this
// tree's build, measured 2026-10-20, each shell in its own scratch dir.

#[test]
fn quoted_operator_byte_is_an_argument() {
    // R1, first operator byte quoted: bash prints the literal and opens
    // nothing; the gate makes fdshell do exactly that (bash parity).
    let dir = scratch("q_op_first");
    let dir = dir.to_str().unwrap();
    for (script, want) in [
        ("echo y 2'>'g2", "y 2>g2\n"),
        ("echo w 1'>'g4", "w 1>g4\n"),
        ("echo a 1\">\"f", "a 1>f\n"),
        ("echo a 1\">f\"g", "a 1>fg\n"),
        ("echo a 1'>>'g", "a 1>>g\n"),
        ("echo a 1'<>'g", "a 1<>g\n"),
        ("echo a '>'f", "a >f\n"),
        ("echo a '>>'f", "a >>f\n"),
        ("echo a '>|'f", "a >|f\n"),
        ("echo a 2'>&1'", "a 2>&1\n"),
    ] {
        assert_word_opens_no_file(dir, script, want);
    }
}

#[test]
fn quoted_clobber_byte_is_an_argument() {
    // The `>|` twin of the same rule, with and without `noclobber`: bash
    // prints `a 1>|f` and opens nothing in both.
    let dir = scratch("q_clobber");
    let dir = dir.to_str().unwrap();
    assert_word_opens_no_file(dir, "echo a 1\">|\"f", "a 1>|f\n");
    assert_word_opens_no_file(dir, "set -o noclobber; echo a 1\">|\"f", "a 1>|f\n");
}

#[test]
fn quoted_dup_byte_makes_the_operand_a_path() {
    // R1's continuation clause: the quoted `&` ends the operator, so the
    // operand is the *word* from that byte — bash opens the files `&1` and
    // `&f` (measured: `&1` empty from fd 2, `&f` = `a` from fd 1).
    let dir = scratch("q_dup_path");
    let dir = dir.to_str().unwrap();
    let (out, err, code) = run(&format!("cd {dir}; echo a 2>\"&\"1"));
    assert_eq!((out.as_str(), code), ("a\n", 0), "stderr={err:?}");
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(dir).join("&1")).unwrap(),
        "",
        "fd 2 opens the file `&1`"
    );
    let (out, err, code) = run(&format!("cd {dir}; echo a >\"&\"f"));
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(dir).join("&f")).unwrap(),
        "a\n",
        "fd 1 opens the file `&f`"
    );
}

#[test]
fn quoted_read_operator_is_an_argument() {
    // `<` first byte quoted: bash prints the literal (rc 0) and `cat` takes
    // the word as its *file argument* — the report names the argument, not a
    // redirect open. rc 1 → rc 0 flips (`echo a '<'f`) and the report-text
    // flip (`cat "<f_in"x`) are the intentional ones (bash parity).
    let dir = scratch("q_read");
    let dir = dir.to_str().unwrap();
    assert_word_opens_no_file(dir, "echo a '<'f", "a <f\n");
    let (out, err, code) = run(&format!("cd {dir}; cat \"<f_in\"x"));
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err:?}");
    assert!(
        err.contains("<f_inx"),
        "the report names the argument: {err:?}"
    );
    // The mixed row: the quoted-`>` word is an argument; the `<` word stays
    // a redirect of the *word* `1h` (unquoted `<`), which fails to open —
    // rc 1 in both shells (bash reports `1h: No such file`, fdshell the
    // redirect open; the message text is fdshell's chain).
    let (out, err, code) = run(&format!("cd {dir}; cat 2'>'g <'1'h"));
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err:?}");
    let files: Vec<_> = std::fs::read_dir(dir).unwrap().collect();
    assert!(files.is_empty(), "the failed open created no file: {err:?}");
}

#[test]
fn quoted_io_number_takes_the_default_descriptor() {
    // R2: quoted digits are no io-number, so `>` / `>|` take their default
    // fd 1 — the file gets the stdout and stdout goes empty (fd 1 really
    // moved). The surviving divergence is the *argument list*: bash splits
    // `'2'>g` into the argument `2` plus the operator, so bash's `g` =
    // `x 2` (POSIX_COMPATIBILITY "Operators are recognized at word starts").
    let dir = scratch("q_fd");
    let dir = dir.to_str().unwrap();
    for (script, file, want) in [
        ("echo x '2'>g", "g", "x\n"),
        ("echo x \"2\">g", "g", "x\n"),
        ("echo a \"2\">|f", "f", "a\n"),
    ] {
        let (out, err, code) = run(&format!("cd {dir}; {script}"));
        assert_eq!((out.as_str(), code), ("", 0), "{script}: stderr={err:?}");
        assert_eq!(
            std::fs::read_to_string(std::path::Path::new(dir).join(file)).unwrap(),
            want,
            "{script}: the default fd 1 moved into `{file}`"
        );
        std::fs::remove_file(std::path::Path::new(dir).join(file)).ok();
    }
}

#[test]
fn quoted_io_number_keeps_the_clobber_parity() {
    // `'1'>|f` is bash-parity today and stays it: the `>|` bytes are unquoted
    // (the redirect stands, the `|` is absorbed by `clobber_word` at byte
    // level too), the quoted digit is no io-number, so fd 1 — the descriptor
    // bash used — is clobbered. bash's `f` = `a 1` is the argument-list
    // divergence; fdshell's `f` = `a`.
    let dir = scratch("q_clobber_parity");
    let dir = dir.to_str().unwrap();
    for script in ["echo a '1'>|f", "set -o noclobber; echo a '1'>|f"] {
        let (out, err, code) = run(&format!("cd {dir}; {script}"));
        assert_eq!((out.as_str(), code), ("", 0), "{script}: stderr={err:?}");
        assert_eq!(
            std::fs::read_to_string(std::path::Path::new(dir).join("f")).unwrap(),
            "a\n",
            "{script}: fd 1 clobbered `f`"
        );
        std::fs::remove_file(std::path::Path::new(dir).join("f")).ok();
    }
}

#[test]
fn bare_quoted_operator_is_an_argument() {
    // `1'>'` is the bare-token shape of R1: rc 1 `invalid redirect syntax`
    // (the operator bailing on its missing operand) flips to rc 0 printing
    // `hi 1>`, the bash parity the task measured.
    let dir = scratch("bare_quoted");
    let dir = dir.to_str().unwrap();
    assert_word_opens_no_file(dir, "echo hi 1'>'", "hi 1>\n");
}

#[test]
fn quoted_third_operator_byte_starts_no_background_job() {
    // The 3-byte arms (`&>>`, `&>&name`, `&>|&name`) gate their third byte
    // too: a word quoting it is an argument — it prints before the next
    // statement and records **no** background job (a shortened gate turns the
    // pidvar words into jobs, whose output arrives after `tail`). bash
    // splits the leading `&` into a control operator here (the word-split
    // family: rc 0/2 with syntax noise, `&p`/`|` files); fdshell's word rule
    // keeps the whole word an argument, measured 2026-10-20.
    let dir = scratch("q_third");
    let dir = dir.to_str().unwrap();
    for (script, want) in [
        ("echo &\">\">f; echo tail", "&>>f\ntail\n"),
        ("echo &\">\"&p; echo tail", "&>&p\ntail\n"),
        ("echo &>\"&\"p; echo tail", "&>&p\ntail\n"),
        ("echo &>\"|\"&p; echo tail", "&>|&p\ntail\n"),
    ] {
        assert_word_opens_no_file(dir, script, want);
    }
}

#[test]
fn quoted_dup_ampersand_after_a_multi_digit_fd_opens_the_path() {
    // `12> "&" x`: the quoted `&` ends the operator, fd 12 opens the file
    // `&x` (empty) — bash creates the same file (measured 2026-10-20).
    let dir = scratch("q_multi_fd");
    let dir = dir.to_str().unwrap();
    let (out, err, code) = run(&format!("cd {dir}; echo a 12>\"&\"x"));
    assert_eq!((out.as_str(), code), ("a\n", 0), "stderr={err:?}");
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(dir).join("&x")).unwrap(),
        "",
        "fd 12 opens the file `&x`"
    );
}

#[test]
fn accepted_quoted_io_number_argument_list_divergences() {
    // What this task does NOT fix (the word-split family): fdshell never
    // splits a word at an operator byte, so the quoted digits never become
    // arguments. The descriptor is bash's (R2) but the argument list is
    // fdshell's, and the files differ from bash's by exactly the digits.
    // Pinned deliberately (POSIX_COMPATIBILITY "Operators are recognized at
    // word starts" owns the family).
    let dir = scratch("q_diverge");
    let dir = dir.to_str().unwrap();
    // bash: `g1 = "x 1"`.
    let (out, err, code) = run(&format!("cd {dir}; echo x '1'>g1"));
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(dir).join("g1")).unwrap(),
        "x\n",
        "bash's g1 = `x 1` (the word-split family)"
    );
    std::fs::remove_file(std::path::Path::new(dir).join("g1")).ok();
    // bash: `f` empty, `g = "a 1 2"` — two redirects, the last to fd 1 wins.
    let (out, err, code) = run(&format!("cd {dir}; echo a '1'>f '2'>g"));
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(dir).join("f")).unwrap(),
        "",
        "the earlier `>f` was opened and truncated"
    );
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(dir).join("g")).unwrap(),
        "a\n",
        "bash's g = `a 1 2`"
    );
    std::fs::remove_file(std::path::Path::new(dir).join("f")).ok();
    std::fs::remove_file(std::path::Path::new(dir).join("g")).ok();
    // bash: `f = "1"` (`printf %s` gets the digit argument).
    let (out, err, code) = run(&format!("cd {dir}; printf %s '1'>f"));
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(dir).join("f")).unwrap(),
        "",
        "bash's f = `1` (the digit is an argument there, not here)"
    );
    // The `2'>'&3` word-split row: bash splits into `2>` + backgrounded `&3`
    // (stdout `a 2>`, rc 127); fdshell keeps the word an argument, rc 0.
    let (out, err, code) = run(&format!("cd {dir}; echo a 2'>'&3"));
    assert_eq!(
        (out.as_str(), code),
        ("a 2>&3\n", 0),
        "bash: `a 2>` + rc 127 (the word-split family), stderr={err:?}"
    );
}

// --- the redirect target is a substitution surface (task #278) --------------
//
// The target word goes through the word pipeline: `$( … )`, `` ` … ` ``,
// `$(( … ))`, `$name`, `${name…}`, `~` expand, the result is split on `IFS`,
// and every field is globbed (README "Redirection"). The target must expand to
// **exactly one** word: 0 words (`> $(true)`) and >1 (`n="a b"; > $n`) are
// bash's `ambiguous redirect`, and an unmatched pattern keeps its literal word
// (`nullglob`/`failglob` are task #369). Every value below is `bash -c` 5.3.9
// and this tree's build, measured 2026-10-21 in a scratch dir seeded with
// `a1 a2 sub/`; the scratch listing is the assert.

/// A scratch dir seeded with `a1 a2 sub/`, the shape the glob rows need.
fn seeded(tag: &str) -> Scratch {
    let dir = scratch(tag);
    for name in ["a1", "a2"] {
        std::fs::write(dir.join(name), b"").unwrap();
    }
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    dir
}

/// The sorted directory listing of `dir`.
fn listing(dir: &str) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_str().unwrap().to_string())
        .collect();
    files.sort();
    files
}

/// Run `script` in `dir` and return its `(stdout, stderr, rc)` plus the sorted
/// directory listing.
fn run_ls(dir: &str, script: &str) -> (String, String, i32, Vec<String>) {
    let (out, err, code) = run(&format!("cd {dir}; {script}"));
    (out, err, code, listing(dir))
}

/// The names `dir` holds beyond the seeded `a1 a2 sub`: what the script created.
fn created(files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|f| !matches!(f.as_str(), "a1" | "a2" | "sub"))
        .cloned()
        .collect()
}

/// Drop every file the rows create, keeping the seed: rows share one dir.
fn clean(dir: &str) {
    for name in created(&listing(dir)) {
        std::fs::remove_file(std::path::Path::new(dir).join(name)).ok();
    }
}

/// The expansions bash runs in a redirection word, each producing exactly one
/// name. `> $(echo f)` is the report's headline: bash writes `f`, and fdshell
/// used to create a file named `$(echo f)`.
#[test]
fn substituted_target_creates_the_expanded_name() {
    let dir = seeded("sub_target");
    let dir = dir.to_str().unwrap();
    for (script, want, stdout) in [
        ("echo hi > $(echo f)", "f", ""),
        ("echo hi >$(echo f)", "f", ""),
        ("echo hi >> $(echo f)", "f", ""),
        ("echo hi >| $(echo f)", "f", ""),
        ("echo hi &> $(echo f)", "f", ""),
        // `<>` with no io-number is fd **0** (POSIX's `<> file` opens it for
        // reading and writing on the input side), so `echo` keeps stdout.
        ("echo hi <> $(echo f)", "f", "hi\n"),
        ("echo hi 2> $(echo f)", "f", "hi\n"),
        ("echo hi > `echo f`", "f", ""),
        ("echo hi > a`echo b`", "ab", ""),
        ("n=f; echo hi > $n", "f", ""),
        ("echo hi > ${n}", "f", ""),
        ("echo hi > pre$(echo mid)post", "premidpost", ""),
        ("echo hi > $(echo a)$(echo b)", "ab", ""),
        ("echo hi > $((1+1))", "2", ""),
        ("echo hi >f$((1))", "f1", ""),
        ("echo hi > a\\*b$(true)", "a*b", ""),
        ("echo hi >\\|$(echo f)", "|f", ""),
    ] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, &format!("n=f; {script}"));
        assert_eq!(
            (out.as_str(), code),
            (stdout, 0),
            "{script}: stderr={err:?}"
        );
        assert_eq!(created(&files), vec![want.to_string()], "{script}");
    }
}

/// The two quoted-target rules bash draws between the quote kinds: double
/// quotes do **not** suppress `$( … )` (`> "$(echo f)"` writes `f`), single
/// quotes do (`> '$(echo f)'` writes `$(echo f)`), and a quoted pattern byte
/// never globs (`> "$p"` with `p='a*'` writes the file `a*`).
#[test]
fn quoted_target_rules_follow_the_quote_kind() {
    let dir = seeded("sub_quoted");
    let dir = dir.to_str().unwrap();
    for (script, want) in [
        ("echo hi > \"$(echo f)\"", "f"),
        ("echo hi > '$(echo f)'", "$(echo f)"),
        ("echo hi > \"$(echo a; echo b)\"", "a\nb"),
        ("p='a*'; echo hi > \"$p\"", "a*"),
        ("echo hi > '~'", "~"),
        ("echo hi > ~\"\"", "~"),
    ] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!((out.as_str(), code), ("", 0), "{script}: stderr={err:?}");
        assert_eq!(created(&files), vec![want.to_string()], "{script}");
    }
}

/// The `ambiguous redirect` arms, 0 and >1 fields alike: nothing is created and
/// rc is 1. `$(echo \*)` shows the expansion being globbed **again**, `a*` the
/// two-match glob the rule has always covered.
#[test]
fn target_expanding_to_zero_or_several_fields_is_ambiguous() {
    let dir = seeded("sub_ambiguous");
    let dir = dir.to_str().unwrap();
    for script in [
        "n=\"a b\"; echo hi > $n",
        "echo hi > $(echo a; echo b)",
        "echo hi > $(true)",
        "n=; echo hi > $n",
        "set --; echo hi > $@",
        "echo hi > $(echo \"f g\")",
        "echo hi > $(echo \\*)",
        "echo hi > a*",
    ] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err:?}");
        assert!(err.contains("ambiguous redirect"), "{script}: {err:?}");
        assert!(created(&files).is_empty(), "{script}: created a file");
    }
}

/// The rc of a failed redirection stays fdshell's, not bash's: the script
/// aborts (`echo after` never runs), where bash skips only the command. Task
/// #179 owns the difference, so this is the pin that keeps it visible.
#[test]
fn ambiguous_redirect_aborts_the_script_and_names_179() {
    let dir = seeded("sub_abort");
    let dir = dir.to_str().unwrap();
    let (out, err, code, files) = run_ls(dir, "echo hi > $(true); echo after");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err:?}");
    assert!(
        !out.contains("after"),
        "bash prints `after` and skips only the command (#179)"
    );
    assert!(created(&files).is_empty());
}

/// The redirect's own glob policy: an unmatched pattern opens the pattern word,
/// `set -f` passes the pattern through. The `nullglob`/`failglob` rows are the
/// accepted divergence — bash deletes/errs the word and reports `ambiguous
/// redirect` / `no match`, fdshell opens the literal pattern (task #369).
#[test]
fn unmatched_pattern_target_keeps_its_literal_word() {
    let dir = seeded("sub_pattern");
    let dir = dir.to_str().unwrap();
    for (script, want) in [
        ("echo hi > zzz*", "zzz*"),
        ("set -f; echo hi > a*", "a*"),
        ("shopt -s nullglob; echo hi > zzz*", "zzz*"),
        ("shopt -s failglob; echo hi > zzz*", "zzz*"),
    ] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!((out.as_str(), code), ("", 0), "{script}: stderr={err:?}");
        assert_eq!(created(&files), vec![want.to_string()], "{script}");
    }
    // One match opens the match (`a1*` → `a1`), and an expansion landing in a
    // subdirectory opens there (`$d/x` → `sub/x`); both write into seeded
    // names, so the created-set stays empty and `cat` is the assert.
    for script in ["d=sub; echo hi > $d/x; cat sub/x", "echo hi > a1*; cat a1"] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!(
            (out.as_str(), code),
            ("hi\n", 0),
            "{script}: stderr={err:?}"
        );
        assert!(created(&files).is_empty(), "{script}: {files:?}");
    }
}

/// The leading `~` of a target expands (bash: `> ~` opens `$HOME` and fails
/// `Is a directory`), a `~name` stays literal (no password database here), and
/// a quoted tilde stays literal (task #269's rule, same mask).
#[test]
fn tilde_in_a_target_expands_with_its_own_rules() {
    let dir = seeded("sub_tilde");
    let dir = dir.to_str().unwrap();
    for (script, want) in [
        ("echo hi > ~x", "~x"),
        ("n=f; echo hi > ~$n", "~f"),
        ("echo hi > ~/sub/x", ""),
    ] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, &format!("HOME={dir}; {script}"));
        if want.is_empty() {
            // `~/sub/x` opens under the expanded HOME: one level deep, so the
            // top-level listing is unchanged and the open succeeds.
            assert_eq!((out.as_str(), code), ("", 0), "{script}: stderr={err:?}");
            assert!(created(&files).is_empty(), "{script}: {files:?}");
            continue;
        }
        assert_eq!((out.as_str(), code), ("", 0), "{script}: stderr={err:?}");
        assert_eq!(created(&files), vec![want.to_string()], "{script}");
    }
    clean(dir);
    let (out, err, code, files) = run_ls(dir, &format!("HOME={dir}; echo hi > ~"));
    assert_eq!(
        (out.as_str(), code),
        ("", 1),
        "bash: `/home/…: Is a directory', rc 1; stderr={err:?}"
    );
    assert!(
        created(&files).is_empty(),
        "the expanded HOME is the dir itself"
    );
}

/// The target expands in the **pipeline child** too, where the open happens in
/// the forked stage: a `$( … )` there forks a grandchild (the shape argument
/// substitution already has in the child).
#[test]
fn pipeline_stage_target_expands_in_the_child() {
    let dir = seeded("sub_pipe");
    let dir = dir.to_str().unwrap();
    let (out, err, code, files) = run_ls(dir, "echo a | echo b > $(echo f)");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    assert_eq!(created(&files), vec!["f".to_string()]);
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(dir).join("f")).unwrap(),
        "b\n"
    );
}

/// `exec >$(echo f)` has no command to run, so the redirection (and its
/// expansion) stays for the rest of the script.
#[test]
fn exec_target_expands_and_stays() {
    let dir = seeded("sub_exec");
    let dir = dir.to_str().unwrap();
    let (out, err, code, files) = run_ls(dir, "exec > $(echo f); echo hi");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    assert_eq!(created(&files), vec!["f".to_string()]);
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(dir).join("f")).unwrap(),
        "hi\n"
    );
}

/// The `%` byte stays out of the target's trigger gate, measured in bash:
/// `> a%b` and `> a%CWD/x` are file names, not fd lookups, and the spaced `%v`
/// operand stays the `%`-namespace guard's syntax error (POSIX_COMPATIBILITY
/// "Operators are recognized at word starts", #323).
///
/// The two rows with a **live** `%log` fd are what pins the gate: `%` alone
/// never triggers the expansion, so `> a%log` keeps its bytes (bash agrees, and
/// expanding the fd would create `a<fd>`), while a word carrying *another*
/// trigger reaches the `%name` arm — the accepted leak task #371 owns, pinned
/// here in the shape that actually diverges (`a<fd>x`, where bash writes
/// `a%logx`).
#[test]
fn percent_bytes_in_a_target_stay_file_name_data() {
    let dir = seeded("sub_percent");
    let dir = dir.to_str().unwrap();
    // Opening `%log` is the fdshell extension bash has no equivalent of; the
    // `openat2` command itself creates `log`, so both names are expected.
    const OPEN: &str =
        "builtin openat2 --flags O_CREAT --flags O_EXCL --flags O_RDWR --mode 0644 log";
    for (script, want) in [
        ("echo hi > a%b", &["a%b"][..]),
        // `%log` unset: the fd name collapses to nothing, so bash and fdshell
        // agree on `a%logx` and the leak is invisible here — the live-fd row
        // below is what pins it.
        ("echo hi > a%log$(echo x)", &["a%logx"]),
        ("echo hi > $(echo \"%log\")", &["%log"]),
        // The `%fd` target the shell *does* own is a parse-time variable
        // source (the attached `>%name` form), never a word to expand.
        (&format!("{OPEN} %>%log; echo x >%log"), &["log"]),
        // `%` is the word's only trigger byte: no expansion, so the fd variable
        // name stays file-name data. `target_trigger -> true` creates `a<fd>`.
        (
            &format!("{OPEN} %>%log; echo hi > a%log"),
            &["a%log", "log"],
        ),
    ] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!((out.as_str(), code), ("", 0), "{script}: stderr={err:?}");
        let want: Vec<String> = want.iter().map(|w| w.to_string()).collect();
        assert_eq!(created(&files), want, "{script}");
    }
    // The #371 divergence, with the fd **open**: a `$` trigger puts the whole
    // word through substitution, so the `%log` name resolves to its fd number.
    // bash writes the literal `a%logx`; fdshell writes `a<fd>x` (the fd number
    // depends on how many fds the process holds, so the shape is asserted).
    clean(dir);
    let (out, err, code, files) = run_ls(dir, &format!("{OPEN} %>%log; echo hi > a%log$(echo x)"));
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let names = created(&files);
    assert_eq!(names.len(), 2, "the target plus `log`: {names:?}");
    let name = names
        .iter()
        .find(|f| f.starts_with('a'))
        .unwrap()
        .to_string();
    let fd_number = name
        .strip_prefix('a')
        .and_then(|n| n.strip_suffix('x'))
        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    assert!(
        fd_number.is_some(),
        "the fd number replaces the `%log` name, where bash writes `a%logx`: {name}"
    );
    clean(dir);
    let (out, err, code, files) = run_ls(dir, "echo hi > a%CWD/x");
    assert_eq!(
        (out.as_str(), code),
        ("", 1),
        "bash: ENOENT for that name, rc 1; stderr={err:?}"
    );
    assert!(created(&files).is_empty(), "{files:?}");
    clean(dir);
    let (out, err, code, files) = run_ls(dir, "printf x > %3");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err:?}");
    assert!(err.contains("invalid redirect"), "the #323 gate: {err:?}");
    assert!(created(&files).is_empty(), "{files:?}");
}

/// Two shapes that stay as they were, pinned so the new surface cannot move
/// them: the `>&N` dup word is **not** expanded (task #370), and a `<>` glued
/// after a non-digit prefix stays an argument (POSIX_COMPATIBILITY "Operators
/// are recognized at word starts"), where bash redirects.
#[test]
fn the_dup_word_and_a_non_digit_prefixed_operator_stay_unexpanded() {
    let dir = seeded("sub_dup");
    let dir = dir.to_str().unwrap();
    clean(dir);
    let (out, err, code, files) = run_ls(dir, "echo hi >&$(echo 1)");
    assert_eq!(
        (out.as_str(), code),
        ("hi >&1\n", 0),
        "bash dups fd 1 and prints `hi` (#370); stderr={err:?}"
    );
    assert!(created(&files).is_empty(), "{files:?}");
    clean(dir);
    let (out, err, code, files) = run_ls(dir, "echo hi<>$(echo f)");
    assert_eq!(
        (out.as_str(), code),
        ("hi<>f\n", 0),
        "bash creates `f` and prints `hi` (the word rule); stderr={err:?}"
    );
    assert!(created(&files).is_empty(), "{files:?}");
}

/// A quoted empty operand stays the operand gate's parse error, where bash
/// opens `''` and fails `ENOENT` (#323 owns the operand shapes): the
/// quoted-empty word rule the target expansion adds is therefore unreachable
/// for the bare and attached forms, and this pin keeps that true.
#[test]
fn quoted_empty_target_stays_a_parse_error() {
    let dir = seeded("sub_empty");
    let dir = dir.to_str().unwrap();
    for script in ["echo hi > \"\"", "echo hi >\"\""] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err:?}");
        assert!(err.contains("invalid redirect"), "{script}: {err:?}");
        assert!(created(&files).is_empty(), "{script}: {files:?}");
    }
}

/// A target whose expansion **fails** keeps its cause in the chain and creates
/// nothing: the arith error rides under `failed to expand the redirect target`
/// (`OpenRedirectError::TargetExpand`) at rc 1, where bash reports the same
/// arithmetic failure at rc 1. An unsupported parameter form is a 0-field
/// expansion, so `${x@}` lands on bash's `ambiguous redirect` instead. The arith
/// rows are task #213's redirect half, measured in bash 5.3.9.
#[test]
fn a_failing_target_expansion_keeps_its_cause() {
    let dir = seeded("sub_expand_fail");
    let dir = dir.to_str().unwrap();
    for (script, needle) in [
        ("echo hi > $((1/0))", "division"),
        ("echo hi > $((x+))", "syntax"),
        ("echo hi > a$((1/0))", "division"),
        ("echo hi > $(echo a)$((1/0))", "division"),
    ] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err:?}");
        assert!(
            err.contains("failed to expand the redirect target"),
            "{script}: the chain must name the target, stderr={err}"
        );
        assert!(err.contains(needle), "{script}: stderr={err}");
        assert!(created(&files).is_empty(), "{script}: created a file");
    }
    clean(dir);
    let (out, err, code, files) = run_ls(dir, "echo hi > ${x@}");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err:?}");
    assert!(err.contains("ambiguous redirect"), "stderr={err}");
    assert!(created(&files).is_empty(), "{files:?}");
}
// --- a redirections-only statement is a command (task #303) ------------------
//
// POSIX #2.9: a simple command may be **redirections only**. The redirections
// apply (the file is created/truncated), the shell's own descriptors come back,
// the status is **0**, and **no fork** happens. fdshell used to resolve the
// operator token as word 0: rc 1 `failed to resolve command path: ">f"` /
// `">f" not found`. Every row below is `bash -c` 5.3.9 and this tree's build,
// measured 2026-10-23 in a scratch dir seeded with `a1 a2 sub` (the `seeded`
// helper above), so the directory listing is part of the assert.
//
// The **no-fork** half is observable: a `NAME=value` prefix **persists**
// (`VAR=x >f` keeps `VAR=x`, as bash, because there is no command for the prefix
// to scope to), and the descriptor-only shapes restore the shell's own fds —
// `tests/herestring.rs::prefixed_here_string_at_a_run_start_is_a_redirection_only_command`
// pins the restore through a closed fd 3.

/// The names `created` reports, from a literal list.
fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

/// Run `script` with the process's working directory set to `dir`. `run_ls`
/// prefixes `cd {dir}; `, and a statement terminated by `&` does not survive
/// that prefix plus a following newline segment (the tokenizer's pre-existing
/// quirk, which `echo a &` hits exactly the same way), so the async row below
/// runs in the directory instead.
fn run_cwd(dir: &str, script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .current_dir(dir)
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

/// Run with the shell's fd 3 **closed from the start** (the same helper as
/// `tests/heredoc.rs` and `tests/herestring.rs`): `sh` closes it before
/// `exec`-ing fdshell, so the fd table the child sees is deterministic whatever
/// the harness inherited. A pin that asserts fd 3 is *closed* in the parent
/// needs this — cargo-mutants hands every test process its jobserver pipes as
/// open fds ≥ 3, which would make a `>&3` dup succeed (LESSONS, task #276).
fn run_fd3_closed(script: &str) -> (String, String, i32) {
    let output = Command::new("sh")
        .args(["-c", "exec 3>&-; exec \"$0\" -c \"$1\"", BIN, script])
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

/// The plain family: every operator shape, in every statement position, is a
/// command that applies its redirections and exits 0 (bash rc 0 in each row).
#[test]
fn a_redirect_only_statement_creates_its_file_and_exits_zero() {
    let dir = seeded("redirect_only");
    let dir = dir.to_str().unwrap();
    let rows: [(&str, &[&str]); 21] = [
        (">f", &["f"]),
        ("> f", &["f"]),
        (">>f", &["f"]),
        (">|f", &["f"]),
        ("2>f", &["f"]),
        ("&>f", &["f"]),
        ("&>>f", &["f"]),
        (">f 2>&1", &["f"]),
        ("> f 2>&1 3>&1", &["f"]),
        (">f >g", &["f", "g"]),
        (">f 2>g", &["f", "g"]),
        (">$(echo f)", &["f"]),
        ("{ >f; }", &["f"]),
        ("if true; then >f; fi", &["f"]),
        ("if false; then >f; else >g; fi", &["g"]),
        ("for i in a; do >f; done", &["f"]),
        ("case a in a) >f;; esac", &["f"]),
        ("true && >f", &["f"]),
        ("false || >f", &["f"]),
        (">f | cat", &["f"]),
        ("cat | >f", &["f"]),
    ];
    for (script, want) in rows {
        clean(dir);
        let (_out, err, code, files) = run_ls(dir, script);
        assert_eq!(
            (code, created(&files)),
            (0, names(want)),
            "{script}: stderr={err:?}"
        );
    }
    // The descriptor-only shapes — a dup, a close, a here-string — need no file
    // at all (bash rc 0, nothing created).
    clean(dir);
    for script in [">&2", ">&-", ">&1", "2>&1", "<<<x", "3<<<x", "0<&-"] {
        let (_out, err, code, files) = run_ls(dir, script);
        assert_eq!(
            (code, created(&files)),
            (0, names(&[])),
            "{script}: stderr={err:?}"
        );
    }
    // The async shape: the background job re-enters the interpreter, so the
    // same arm runs in the forked job and the file appears (bash rc 0).
    let dir = scratch("redirect_only_async");
    let dir = dir.to_str().unwrap();
    let (out, err, code) = run_cwd(dir, ">f &\nwait");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    assert!(std::path::Path::new(dir).join("f").exists(), "{dir}");
}

/// The status is 0 and the statement is a full member of the lists: `$?`, the
/// `&&` / `||` chains, the `;` segments, the scoped prefix (which **persists**,
/// as bash keeps `VAR=x` from `VAR=x >f`), and the plain-command regression
/// row.
#[test]
fn a_redirect_only_statement_leaves_the_status_and_the_prefix() {
    let dir = seeded("redirect_only_status");
    let dir = dir.to_str().unwrap();
    let rows: [(&str, &str, &[&str]); 7] = [
        (">f; echo $?", "0\n", &["f"]),
        (">f && >g && echo both", "both\n", &["f", "g"]),
        ("true && >f && echo ok", "ok\n", &["f"]),
        (">f; >g; echo done", "done\n", &["f", "g"]),
        ("echo one; >f; echo two", "one\ntwo\n", &["f"]),
        ("VAR=x >f; echo $VAR; echo $?", "x\n0\n", &["f"]),
        ("echo a >f", "", &["f"]),
    ];
    for (script, stdout, want) in rows {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!(
            (out.as_str(), code, created(&files)),
            (stdout, 0, names(want)),
            "{script}: stderr={err:?}"
        );
    }
}

/// A redirection that cannot open its target fails the statement at rc 1 with a
/// chain naming the path, and creates nothing (bash rc 1 too, with its own
/// `No such file or directory`). fdshell opens **every** target before applying
/// any, so `>f <nofile` leaves no `f` where bash does — the same all-or-nothing
/// open `: >f <nofile` has always had here. A target expanding to no word, or
/// to two, is bash's `ambiguous redirect`; `<file` on an existing file is the
/// success case of the same shape.
#[test]
fn a_redirect_only_statement_with_an_unopenable_target_fails() {
    let dir = seeded("redirect_only_fail");
    let dir = dir.to_str().unwrap();
    for script in ["<nofile", ">f <nofile", "2<nofile"] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err:?}");
        assert!(err.contains("redirection"), "{script}: stderr={err:?}");
        assert!(created(&files).is_empty(), "{script}: created {files:?}");
    }
    // The `>&3` family takes its source from the **parent's** fd table, so the
    // rows only fail when fd 3 is closed there (bash: rc 1 `3: Bad file
    // descriptor`, nothing created). They run through `run_fd3_closed`: under a
    // harness that inherited fd 3 — cargo-mutants hands every test process its
    // jobserver pipes as fds ≥ 3, LESSONS task #276 — the dup would succeed and
    // both statements would be rc 0 and silent.
    for script in [">&3", "2>&3"] {
        clean(dir);
        let (out, err, code) = run_fd3_closed(&format!("cd {dir}; {script}"));
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err:?}");
        assert!(err.contains("redirection"), "{script}: stderr={err:?}");
        let files = listing(dir);
        assert!(created(&files).is_empty(), "{script}: created {files:?}");
    }
    clean(dir);
    for script in [">$(echo)", "> $(true)", "n='a b'; >$n"] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err:?}");
        assert!(err.contains("ambiguous redirect"), "{script}: {err}");
        assert!(created(&files).is_empty(), "{script}: created {files:?}");
    }
    let dir = scratch("redirect_only_read");
    let dir = dir.to_str().unwrap();
    std::fs::write(std::path::Path::new(dir).join("pre"), b"x").unwrap();
    let (out, err, code) = run(&format!("cd {dir}; < pre; echo $?"));
    assert_eq!((out.as_str(), code), ("0\n", 0), "stderr={err:?}");
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(dir).join("pre")).unwrap(),
        "x"
    );
}

/// A word **behind** the redirections is the command word: bash runs `g` with
/// the redirect (rc 127 there), fdshell reports its shell-wide not-found **1**
/// and the file is created all the same. A `%` word keeps the collection loop's
/// own reading: the `%tag>%name` capture operator has no command to attach to
/// and is dropped (rc 0), while `%o` is no capture operator at all, so it is
/// the command word (bash reports job-control noise, rc 1, on both).
#[test]
fn a_redirect_only_statement_with_a_word_runs_that_word() {
    let dir = seeded("redirect_only_word");
    let dir = dir.to_str().unwrap();
    let rows: [(&str, &str, &[&str]); 4] = [
        (">f g", "\"g\" not found", &["f"]),
        (">f /nope/x", "\"/nope/x\" not found", &["f"]),
        (">f g h", "\"g\" not found", &["f"]),
        (">f %o", "\"%o\" not found", &["f"]),
    ];
    for (script, needle, want) in rows {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err:?}");
        assert!(err.contains(needle), "{script}: stderr={err:?}");
        assert_eq!(created(&files), names(want), "{script}");
    }
    clean(dir);
    let (out, err, code, files) = run_ls(dir, ">f %t>%v");
    assert_eq!(
        (out.as_str(), code, created(&files)),
        ("", 0, names(&["f"])),
        "stderr={err:?}"
    );
}

/// The guards: a **quoted** operator word stays a command word (the
/// operator-quoting gate, task #250), a quoted empty word 0 stays a command
/// lookup (bash 127, `f` created by the redirection that still applies), an
/// io-number must be glued to its operator (`2 > f` is the command `2`), a bare
/// operator with no operand stays the parse error, and every here-doc-bearing
/// statement keeps the leading-operator behavior the #185 pins record (owners
/// #345 and the here-doc family — bash is rc 2 on the `> <<EOF` rows, rc 0 on
/// the last two).
#[test]
fn a_quoted_operator_or_heredoc_word_stays_a_command() {
    let dir = seeded("redirect_only_guard");
    let dir = dir.to_str().unwrap();
    for script in ["'>f'", "'&>f'", "\"2>f\"", "\">f\"", "'>>f'", "'<<<x'"] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err:?}");
        assert!(
            err.contains("failed to resolve command path"),
            "{script}: stderr={err:?}"
        );
        assert!(created(&files).is_empty(), "{script}: {files:?}");
    }
    // A quoted empty word 0 with a redirection: the lookup fails at rc 1 (bash
    // 127) and the redirection still applies — the run-side arm is gated on
    // `command_quoted == false`, which is what keeps this shape out of it.
    clean(dir);
    let (out, err, code, files) = run_ls(dir, "\"\" >f");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err:?}");
    assert!(err.contains("\"\" not found"), "stderr={err:?}");
    assert_eq!(created(&files), names(&["f"]));
    // The io-number must be glued: `2 > f` is the command `2` with a redirect.
    clean(dir);
    let (out, err, code, files) = run_ls(dir, "2 > f");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err:?}");
    assert!(err.contains("\"2\" not found"), "stderr={err:?}");
    assert_eq!(created(&files), names(&["f"]));
    // A bare operator with no operand is the parse error (bash rc 2).
    for script in [">", ">>", "<", "&>", "> >"] {
        clean(dir);
        let (out, err, code, files) = run_ls(dir, script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err:?}");
        assert!(err.contains("invalid redirect"), "{script}: {err}");
        assert!(created(&files).is_empty(), "{script}: {files:?}");
    }
    // The here-doc family keeps its owners: the operator word is reported and
    // the body lines never run.
    let rows: [(&str, &str); 5] = [
        ("> <<EOF\nbody\nEOF", "\">\" not found"),
        (">| <<EOF\nbody\nEOF", "\">|\" not found"),
        (">| x <<EOF\nbody\nEOF", "\">|\" not found"),
        (">x <<EOF\nbody\nEOF", "\">x\" not found"),
        ("cat <<A & <<B\nx\nA\ny\nB", "\"<<B\" not found"),
    ];
    for (script, needle) in rows {
        // `run_cwd`, not `run_ls`: the `cat <<A & <<B` row is an `&`-terminated
        // segment followed by a newline, which the `cd {dir}; ` prefix breaks
        // (the tokenizer quirk `run_cwd` exists for). Its stdout is the prefix
        // job's body (`x`), which `tests/herestring.rs` pins; rc 1 is the
        // suffix's report.
        let (_out, err, code) = run_cwd(dir, script);
        assert_eq!(code, 1, "{script}: stderr={err:?}");
        assert!(err.contains(needle), "{script}: stderr={err:?}");
    }
}

/// `>>&x` is the collection loop's own vocabulary: `>>` with the **path**
/// operand `&x` (fdshell never splits a word at an operator byte, so
/// `echo a >>&x` appends to the file `&x` today). Task #303 inherits that
/// reading at word 0, so the statement is an rc-0 redirections-only command;
/// bash rejects the word outright (rc 2, `syntax error near unexpected token
/// '&'`). The `>&x` twin is no redirect at all — the dup arm answers `None` for
/// a non-numeric, non-`%` tail — so it stays the command word.
#[test]
fn an_ampersand_operand_follows_the_loops_own_vocabulary() {
    let dir = seeded("redirect_only_amp");
    let dir = dir.to_str().unwrap();
    clean(dir);
    let (out, err, code, files) = run_ls(dir, ">>&x");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    assert_eq!(created(&files), names(&["&x"]));
    clean(dir);
    let (out, err, code, files) = run_ls(dir, ">&x");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err:?}");
    assert!(err.contains("\">&x\" not found"), "stderr={err:?}");
    assert!(created(&files).is_empty(), "{files:?}");
    // The pid-var background forms are the same token family, and with no
    // command word they launch nothing: the statement is rc 0 and the task is
    // registered under its name (bash rejects both words outright, rc 2).
    let (out, err, code) = run_cwd(dir, "&>&x; echo rc=$?; waitpid &x; echo wp=$?");
    assert_eq!((out.as_str(), code), ("rc=0\nwp=0\n", 0), "stderr={err:?}");
}
