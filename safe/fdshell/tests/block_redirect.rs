#![allow(clippy::unwrap_used)]
//! A redirection written **after** a keyword block's closing keyword belongs to
//! the whole block (task #298): the block's output lands in the target, the
//! shell's own fds are back when the block ends, and the scope opens before the
//! block's own word list / condition runs. That is exactly the rule
//! `{ …; } > f` already implements, and every expectation below is bash 5.3.9's
//! own output for the same script — except the rows that name their owner task
//! (the rc-convention family #179, the word-behind-the-closer family #405, the
//! here-doc / `&>` families #408 / #409, the saved-copy floor #184).

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). Removal is `.ok()`-tolerant: teardown never panics.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("fdshell-blockred-{}-{}", std::process::id(), c));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Run `script` with the scratch dir as its cwd, stdout and stderr **separate**:
/// the family is about which fd the block's output reaches.
fn run(dir: &Path, script: &str) -> Output {
    Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

fn out(o: &Output) -> &str {
    str::from_utf8(&o.stdout).unwrap()
}

fn err(o: &Output) -> &str {
    str::from_utf8(&o.stderr).unwrap()
}

fn rc(o: &Output) -> Option<i32> {
    o.status.code()
}

/// A file in the scratch dir, as bytes: `None` when it was never created.
fn file(dir: &Path, name: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join(name)).ok()
}

/// The five keyword blocks, each writing one line, each with the same trailing
/// redirection: the file holds the block's output, stdout stays empty.
#[test]
fn every_keyword_block_takes_its_trailing_redirect() {
    for (script, want) in [
        ("while :; do echo a; break; done > f", "a\n"),
        ("until false; do echo u; break; done > f", "u\n"),
        ("if true; then echo t; fi > f", "t\n"),
        ("if false; then :; else echo e; fi > f", "e\n"),
        ("if false; then :; elif true; then echo l; fi > f", "l\n"),
        ("for x in a b; do echo $x; done > f", "a\nb\n"),
        ("for ((i=0;i<2;i++)); do echo $i; done > f", "0\n1\n"),
        ("case x in x) echo C;; esac > f", "C\n"),
        ("case x in y) echo N;; *) echo D;; esac > f", "D\n"),
    ] {
        let dir = Scratch::new();
        let o = run(&dir, script);
        assert_eq!(
            (out(&o), rc(&o)),
            ("", Some(0)),
            "{script}: stderr={}",
            err(&o)
        );
        assert_eq!(
            file(&dir, "f"),
            Some(want.as_bytes().to_vec()),
            "{script}: bash puts the block's output in `f`"
        );
    }
}

/// `>>` appends to a target the block did not create, in the same statement.
#[test]
fn append_after_a_closer_appends_to_the_block_target() {
    let dir = Scratch::new();
    let o = run(&dir, "echo pre > f; while :; do echo a; break; done >> f");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(file(&dir, "f"), Some(b"pre\na\n".to_vec()));
}

/// Several redirections to one fd: the **last** one wins, and every entry still
/// opens and truncates its target (bash: `f` empty, `g` = `a`).
#[test]
fn the_last_redirect_to_a_fd_wins_and_the_others_are_truncated() {
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done > f > g");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(file(&dir, "f"), Some(b"".to_vec()));
    assert_eq!(file(&dir, "g"), Some(b"a\n".to_vec()));
}

/// An io-number target, a dup, a close, and a read: each fd does what the
/// operator says, and the shell's own fds are back afterwards.
#[test]
fn io_number_dup_close_and_read_targets() {
    let dir = Scratch::new();
    // `3> f`: fd 3 goes to the file, stdout keeps the block's output.
    let o = run(&dir, "while :; do echo a; break; done 3> f; echo tail");
    assert_eq!(
        (out(&o), rc(&o)),
        ("a\ntail\n", Some(0)),
        "stderr={}",
        err(&o)
    );
    assert_eq!(file(&dir, "f"), Some(b"".to_vec()));

    // `>&2`: the block's output goes to stderr, stdout stays empty.
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done >&2");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(err(&o), "a\n");

    // `>&-`: stdout is closed for the block, so its write fails (bash's own
    // message, same words, no `bash: line 1:` prefix to imitate).
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done >&-");
    assert_eq!((rc(&o), out(&o)), (Some(0), ""), "stderr={}", err(&o));
    assert!(
        err(&o).contains("write error: Bad file descriptor"),
        "the closed stdout surfaces the write error: stderr={}",
        err(&o)
    );

    // `< in`: the block's stdin is the file, and the two redirections combine.
    let dir = Scratch::new();
    let o = run(
        &dir,
        "echo seed > in; while :; do cat; break; done < in > f",
    );
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(file(&dir, "f"), Some(b"seed\n".to_vec()));

    // `>&2 3> f`, `> f 2> g`: the fds split as written.
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done >&2 3> f");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(err(&o), "a\n");
    assert_eq!(file(&dir, "f"), Some(b"".to_vec()));

    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done > f 2> g");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(file(&dir, "f"), Some(b"a\n".to_vec()));
    assert_eq!(file(&dir, "g"), Some(b"".to_vec()));

    // `>|` is the clobber form, `<&0 > f` a dup followed by a write.
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done >| f");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(file(&dir, "f"), Some(b"a\n".to_vec()));
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done <&0 > f");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(file(&dir, "f"), Some(b"a\n".to_vec()));
}

/// The scope is undone when the block ends: the command after it writes to the
/// shell's own stdout, and a `/dev/fd/N` target keeps the output on stdout.
#[test]
fn the_shell_fds_are_back_after_the_block() {
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done > f; echo tail");
    assert_eq!((out(&o), rc(&o)), ("tail\n", Some(0)), "stderr={}", err(&o));
    assert_eq!(file(&dir, "f"), Some(b"a\n".to_vec()));

    let dir = Scratch::new();
    let o = run(&dir, "for x in a; do echo A; done > /dev/fd/2");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(err(&o), "A\n");
}

/// An **empty word list** runs no body, and the target is still created —
/// empty (bash's answer).
#[test]
fn an_empty_word_list_creates_the_target_empty() {
    let dir = Scratch::new();
    let o = run(&dir, "for x in; do echo A; done > f");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(file(&dir, "f"), Some(b"".to_vec()));
}

/// A failing open aborts the block **before** its own evaluation: the `for`
/// word list and the `case` word are not expanded, so their side effects
/// (`touch m`) never happen (bash's order, measured). fdshell stops the `-c`
/// script at rc 1 where bash reports the command's failure and continues — the
/// standing rc convention of task **#179**, pinned by name, never fixed in
/// passing.
#[test]
fn a_failing_open_precedes_the_blocks_own_word_expansion() {
    for (script, closer) in [
        (
            "for x in $(touch m; echo a); do :; done > /nope/x; echo tail",
            "for",
        ),
        (
            "case $(touch m; echo x) in x) echo C;; esac > /nope/x; echo tail",
            "case",
        ),
        (
            "for ((i=$(touch m; echo 1);i<2;i++)); do :; done > /nope/x; echo tail",
            "arith-for",
        ),
        (
            "while $(touch m; true); do :; break; done > /nope/x; echo tail",
            "while",
        ),
    ] {
        let dir = Scratch::new();
        let o = run(&dir, script);
        assert_eq!(
            rc(&o),
            Some(1),
            "{closer}: stdout={:?} stderr={:?}",
            out(&o),
            err(&o)
        );
        assert_eq!(out(&o), "", "{closer}: `tail` never prints (task #179)");
        assert!(
            file(&dir, "m").is_none(),
            "{closer}: the word list is not expanded"
        );
        assert!(err(&o).contains("redirection failed"), "stderr={}", err(&o));
    }
}

/// A here-doc **in the body** plus a trailing redirection: the body the
/// iteration reads goes into the file, stdout stays empty (the row
/// `tests/heredoc.rs` pins for the whole family).
#[test]
fn a_body_heredoc_and_a_trailing_redirect_fill_the_target() {
    for (script, want) in [
        ("while :; do cat <<T; break; done > f\nbodyT\nT", "bodyT\n"),
        ("for x in a; do cat <<T; done > f\nbodyT\nT", "bodyT\n"),
        ("case x in x) cat <<T;; esac > f\nbodyT\nT", "bodyT\n"),
        ("if true; then cat <<T; fi > f\nbodyT\nT", "bodyT\n"),
    ] {
        let dir = Scratch::new();
        let o = run(&dir, script);
        assert_eq!(
            (out(&o), rc(&o)),
            ("", Some(0)),
            "{script}: stderr={}",
            err(&o)
        );
        assert_eq!(file(&dir, "f"), Some(want.as_bytes().to_vec()), "{script}");
    }
}

/// The same rule through every statement shape that reaches a block: the
/// mid-run `&&` / `||` twins, a pipeline stage, and the `time` prefix (whose
/// hook hands the inner block the whole text).
#[test]
fn midrun_pipeline_and_stage_shapes_take_the_redirect() {
    for (script, want) in [
        ("true && while :; do echo a; break; done > f", "a\n"),
        ("false || while :; do echo a; break; done > f", "a\n"),
        (
            "true && for ((i=0;i<2;i++)); do echo $i; done > f",
            "0\n1\n",
        ),
        ("false || case x in x) echo C;; esac > f", "C\n"),
        ("echo a | while :; do echo b; break; done > f", "b\n"),
        ("echo a | if :; then echo b; fi > f", "b\n"),
    ] {
        let dir = Scratch::new();
        let o = run(&dir, script);
        assert_eq!(
            (out(&o), rc(&o)),
            ("", Some(0)),
            "{script}: stderr={}",
            err(&o)
        );
        assert_eq!(file(&dir, "f"), Some(want.as_bytes().to_vec()), "{script}");
    }
    let dir = Scratch::new();
    let o = run(&dir, "time while :; do echo a; break; done > f");
    assert_eq!(rc(&o), Some(0), "stderr={}", err(&o));
    assert_eq!(
        file(&dir, "f"),
        Some(b"a\n".to_vec()),
        "bash files the block's output and prints the timing report to stderr"
    );
}

/// An `exec > z` inside the body is undone by the scope's restore, exactly as
/// for the group twin `{ exec > z; } > f`: bash leaves both files empty with
/// stdout intact.
#[test]
fn the_restore_undoes_an_exec_in_the_body() {
    let dir = Scratch::new();
    let o = run(&dir, "while :; do exec > z; break; done > f; echo tail");
    assert_eq!((out(&o), rc(&o)), ("tail\n", Some(0)), "stderr={}", err(&o));
    assert_eq!(file(&dir, "f"), Some(b"".to_vec()));
    assert_eq!(file(&dir, "z"), Some(b"".to_vec()));
}

/// A closer-shaped word inside a block's own word data stays word data, with a
/// trailing redirection on the block: the body's `echo done` and a `done`
/// pattern are words, not the closer. (The `for x in done>f` spelling is
/// fdshell's word-list leniency — bash rejects the line at rc 2, the family of
/// task **#405** — what is pinned here is that the glue does not steal the
/// word.)
#[test]
fn closer_shaped_words_stay_word_data() {
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo done; break; done > f");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(file(&dir, "f"), Some(b"done\n".to_vec()));

    let dir = Scratch::new();
    let o = run(&dir, "case x in done) echo P;; esac > f");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(
        file(&dir, "f"),
        Some(b"".to_vec()),
        "the `done` pattern does not match `x`"
    );

    let dir = Scratch::new();
    let o = run(&dir, "for x in done>f; do echo A; done > f2");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(file(&dir, "f2"), Some(b"A\n".to_vec()));
    assert!(
        file(&dir, "f").is_none(),
        "the word-list word opens nothing"
    );
}

/// The pinned divergences, each with its owner. A plain word behind the closer
/// is accepted (bash: rc 2), owned by **#405**; a here-doc / here-string
/// operator on the closer is dropped (bash feeds the body to the block),
/// owned by **#408**; the `&>` family on a closer is dropped (bash combines),
/// owned by **#409**; a bare `>` with no operand is fdshell's rc-1 redirect
/// syntax error (bash: rc 2) — the house parse-error convention.
#[test]
fn the_pinned_divergences_behind_a_closer() {
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done x");
    assert_eq!((out(&o), rc(&o)), ("a\n", Some(0)), "stderr={}", err(&o));
    assert!(!dir.join("x").exists(), "#405: the word is not a target");

    let dir = Scratch::new();
    let o = run(&dir, "if true; then echo t; fi x#y && echo y");
    assert_eq!((out(&o), rc(&o)), ("t\ny\n", Some(0)), "stderr={}", err(&o));

    // #408: bash feeds the here-doc body to the block; fdshell prints nothing.
    let dir = Scratch::new();
    let o = run(&dir, "while :; do cat; break; done <<T\nbodyT\nT");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done <<< hi");
    assert_eq!((out(&o), rc(&o)), ("a\n", Some(0)), "stderr={}", err(&o));

    // #409: bash combines stdout and stderr into `f`; fdshell drops the tail.
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done &> f");
    assert_eq!((out(&o), rc(&o)), ("a\n", Some(0)), "stderr={}", err(&o));
    assert!(
        file(&dir, "f").is_none(),
        "#409: the `&>` tail is not attached"
    );

    // The house convention: rc 1 for a parse error, where bash prints rc 2.
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done >");
    assert_eq!((out(&o), rc(&o)), ("", Some(1)), "stderr={}", err(&o));
    assert!(
        err(&o).contains("invalid redirect syntax"),
        "stderr={}",
        err(&o)
    );
}

/// The saved copy is a script-nameable fd, so a body that closes it fails the
/// restore at rc 1: the block twin of
/// `tests/redirect_scope.rs::body_that_closes_the_saved_copy_fails_the_call`,
/// whose floor fix is owned by **#184** (bash: rc 0). The body's output is
/// already in the file when the restore fails.
#[test]
fn a_body_that_closes_the_saved_copy_fails_the_block() {
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; exec 6>&-; break; done > f");
    assert_eq!(rc(&o), Some(1), "stdout={:?} stderr={:?}", out(&o), err(&o));
    assert_eq!(out(&o), "");
    assert!(
        err(&o).contains("redirection failed"),
        "the report is the redirect error: stderr={}",
        err(&o)
    );
    assert_eq!(file(&dir, "f"), Some(b"a\n".to_vec()));
}
