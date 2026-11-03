#![allow(clippy::unwrap_used)]
//! A redirection written **after** a keyword block's closing keyword belongs to
//! the whole block (task #298): the block's output lands in the target, the
//! shell's own fds are back when the block ends, and the scope opens before the
//! block's own word list / condition runs. That is exactly the rule
//! `{ …; } > f` already implements, and every expectation below is bash 5.3.9's
//! own output for the same script — except the rows that name their owner task
//! (the rc-convention family #179, the word-behind-the-closer family #405, the
//! `&>` family #409, the saved-copy floor #184). Task **#408** landed the
//! here-doc / here-string half of the same tail; its matrix is
//! `a_here_doc_or_here_string_on_a_closer_feeds_the_block`.

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

/// The **glued** closer — no space between the keyword and the operator — is
/// the same block: `done>f`, `fi>f`, `esac>f` close the block and the operator
/// redirects it (`while … done>f` was bash's shape all along).
#[test]
fn a_glued_closer_takes_its_redirection() {
    for (script, want) in [
        ("while :; do echo a; break; done>f", "a\n"),
        ("until false; do echo u; break; done>f", "u\n"),
        ("if :; then echo t; fi>f", "t\n"),
        ("for x in a; do echo $x; done>f", "a\n"),
        ("for ((i=0;i<2;i++)); do echo $i; done>f", "0\n1\n"),
        ("case x in x) echo C;; esac>f", "C\n"),
        ("while :; do echo a; break; done>>f", "a\n"),
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
    // A *chained* glued redirection (`done>f>g`) is the word-tokenizer's
    // pre-existing gap, not the block tail's: `echo a>f>g` files nothing on
    // master either, while bash gives `f` empty and `g` the output. fdshell
    // files the block's output in a literal `f>g`, and the block does run.
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done>f>g");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(
        file(&dir, "f>g"),
        Some(b"a\n".to_vec()),
        "the whole glued word became the target (`echo a>f>g` behaves the same)"
    );
    assert!(file(&dir, "f").is_none() && file(&dir, "g").is_none());
    let dir = Scratch::new();
    std::fs::write(dir.join("in"), b"IN\n").unwrap();
    let o = run(&dir, "while :; do cat; break; done<in");
    assert_eq!((out(&o), rc(&o)), ("IN\n", Some(0)), "stderr={}", err(&o));

    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done>f; echo tail");
    assert_eq!((out(&o), rc(&o)), ("tail\n", Some(0)), "stderr={}", err(&o));
    assert_eq!(
        file(&dir, "f"),
        Some(b"a\n".to_vec()),
        "the glue ends the block, the statement after it runs"
    );

    // A glued closer with a **quoted** operand is still rejected at the
    // byte-level word/quote scan, before any block tail runs: bash files the
    // block's output, fdshell reports the missing closer. Measured identical on
    // `master`, so this is not a regression of the glue work — the shape is
    // owned by task **#413**.
    for (script, closer) in [
        (
            "while :; do echo a; break; done>\"f\"",
            "'while' is missing its closing 'done'",
        ),
        (
            "if :; then echo t; fi>\"f\"",
            "'if' is missing its closing 'fi'",
        ),
        (
            "case x in x) echo C;; esac>\"f\"",
            "'case' is missing its closing 'esac'",
        ),
        (
            "while :; do echo a; break; done>>\"f\"",
            "'while' is missing its closing 'done'",
        ),
        (
            "while :; do echo a; break; done>\"$(echo f)\"",
            "'while' is missing its closing 'done'",
        ),
    ] {
        let dir = Scratch::new();
        let o = run(&dir, script);
        assert_eq!(
            (out(&o), rc(&o)),
            ("", Some(1)),
            "{script}: stderr={}",
            err(&o)
        );
        assert!(err(&o).contains(closer), "#413: stderr={}", err(&o));
        assert!(
            file(&dir, "f").is_none(),
            "#413: the block never runs, so `f` is not created: {script}"
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
/// redirection on the block — glued closer included: the body's `echo done` and
/// a `done` pattern are words, not the closer. The `for` **word list** is word
/// data too (task #306), and a list word **glued to an operator** (`done>f`,
/// `done|cat`, `done&&true`) is word data here: bash rejects those spellings
/// outright (rc 2), fdshell runs the statement — the accepted divergence family
/// `tests/cond_blocks.rs::for_list_with_and_is_an_accepted_divergence`
/// (`for i in a && b`) already belongs to, and the rows below are its measured
/// values, not bash's.
#[test]
fn closer_shaped_words_stay_word_data() {
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo done; break; done>f");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(
        file(&dir, "f"),
        Some(b"done\n".to_vec()),
        "the body's `done` is data, the glued `done>` is the closer"
    );

    let dir = Scratch::new();
    let o = run(&dir, "case x in done) echo P;; esac>f");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert_eq!(
        file(&dir, "f"),
        Some(b"".to_vec()),
        "the `done` pattern does not match `x`"
    );

    // The `for` list's glued `done>f` is word data, but the `>` now **splits**
    // the keyword word (task #396): the list is `done` and `>f`, the loop runs
    // both words, and the closer's redirect files both prints (bash stops at
    // rc 2 and opens nothing).
    let dir = Scratch::new();
    let o = run(&dir, "for x in done>f; do echo A; done>f2");
    assert_eq!((out(&o), rc(&o)), ("", Some(0)), "stderr={}", err(&o));
    assert!(
        file(&dir, "f").is_none(),
        "the list word's `>f` is word data: nothing is opened"
    );
    assert_eq!(
        file(&dir, "f2"),
        Some(b"A\nA\n".to_vec()),
        "the split list runs `done` and `>f` (`echo a>f>g` behaves the same)"
    );

    // The family, each row measured against bash 5.3.9: every glued shape runs
    // (rc 0, `A`) where bash stops at rc 2, and the operator-shaped list words
    // (`>f`, `2>/dev/null`, `done2>f`, `<f`) are the pre-existing rows of the
    // same family — unchanged by task #306, listed so the family is one place.
    // The glued **keyword** words now split at the metacharacter (task #396),
    // so the first row's list is `done` and `>f` and the loop prints twice;
    // `done2>f` is no keyword word and stays one word. The spaced spelling is
    // bash parity: the closer's redirect files the list word the loop iterated.
    for (script, want, want_f) in [
        ("for x in done>f; do echo A; done", "A\nA\n", None),
        ("for x in done|cat; do echo A; break; done", "A\n", None),
        ("for x in done&&true; do echo A; break; done", "A\n", None),
        ("for x in >f; do echo A; break; done", "A\n", None),
        ("for x in 2>/dev/null; do echo A; break; done", "A\n", None),
        ("for x in done2>f; do echo A; break; done", "A\n", None),
        ("for x in <f; do echo A; break; done", "A\n", None),
        (
            "for x in done; do echo A; break; done > f",
            "",
            Some(b"A\n".to_vec()),
        ),
    ] {
        let dir = Scratch::new();
        let o = run(&dir, script);
        assert_eq!(
            (out(&o), rc(&o)),
            (want, Some(0)),
            "{script}: stderr={}",
            err(&o)
        );
        assert_eq!(file(&dir, "f"), want_f, "{script}");
    }

    // The closer is the **last** token the closer rule matches (that is what
    // makes the `echo done` body above a word), so in the fdshell-only event
    // `wait` block the tail's own operand can be taken for the closer: with
    // `done > done` the arm pattern loses its `)` and the block is rejected
    // loudly, not mis-run. `master` rejects the same line (`expected 'done'`),
    // so this is the boundary of the rule, not a regression.
    let dir = Scratch::new();
    let o = run(&dir, "wait\nreadable %rd) echo hi ;;\ndone > done");
    assert_eq!(rc(&o), Some(1), "stdout={:?} stderr={:?}", out(&o), err(&o));
    assert!(
        err(&o).contains("missing ')' after arm pattern"),
        "the tail's `done` is read as the closer: stderr={}",
        err(&o)
    );
    assert!(file(&dir, "done").is_none());

    // Beside it: a tail whose operand is not closer-shaped parses, the scope
    // opens (the target is created), and only the unset `%rd` fd fails the round.
    let dir = Scratch::new();
    let o = run(&dir, "wait\nreadable %rd) echo hi ;;\ndone > out");
    assert_eq!(rc(&o), Some(1), "stdout={:?} stderr={:?}", out(&o), err(&o));
    assert!(
        err(&o).contains("rd is not set"),
        "the block parses; the round needs an open `%rd`: stderr={}",
        err(&o)
    );
    assert_eq!(
        file(&dir, "out"),
        Some(b"".to_vec()),
        "`master` rejects this line outright (`expected 'done'`); the tail now belongs to the block"
    );
}

/// The pinned divergences, each with its owner. A plain word behind the closer is
/// the **house parse error at rc 1** (bash: rc 2) — task **#405** landed it, the
/// block runs nothing, and the wording is the keyword-block twin of the group's
/// `group: unexpected word after '}'`; the here-doc / here-string family on a
/// closer is **parity** now (task **#408** landed — the two rows below are the
/// ones that used to be dropped, kept here so the flip is visible, the matrix is
/// in `a_here_doc_or_here_string_on_a_closer_feeds_the_block`); the `&>` family
/// on a closer is dropped (bash combines), owned by **#409**; a bare `>` with no
/// operand is fdshell's rc-1 redirect syntax error (bash: rc 2) — the house
/// parse-error convention.
/// Every value below is measured on this build against bash 5.3.9.
#[test]
fn the_pinned_divergences_behind_a_closer() {
    // #405: the word behind the closer is the report, and nothing of the block
    // runs (bash stops on the whole line, so a statement that already ran is the
    // only output the house convention allows).
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done x");
    assert_eq!((out(&o), rc(&o)), ("", Some(1)), "stderr={}", err(&o));
    assert!(
        err(&o).contains("'done' must be followed by a separator"),
        "stderr={}",
        err(&o)
    );
    assert!(!dir.join("x").exists(), "#405: the word is not a target");

    let dir = Scratch::new();
    let o = run(&dir, "if true; then echo t; fi x#y && echo y");
    assert_eq!((out(&o), rc(&o)), ("", Some(1)), "stderr={}", err(&o));
    assert!(
        err(&o).contains("'fi' must be followed by a separator"),
        "stderr={}",
        err(&o)
    );

    // The same rule on the closer of a block that is itself a body statement —
    // the inner builder answers it. Bash: rc 2, nothing runs.
    let dir = Scratch::new();
    let o = run(
        &dir,
        "while :; do if :; then :; fi x; break; done; echo end",
    );
    assert_eq!((out(&o), rc(&o)), ("", Some(1)), "stderr={}", err(&o));
    assert!(
        err(&o).contains("'fi' must be followed by a separator"),
        "stderr={}",
        err(&o)
    );
    // A body that never runs never reports (the shared lazy-parse rule), so the
    // identical shape with `while false` stays bash's one divergence: fdshell runs
    // `echo end` at rc 0, bash prints nothing at rc 2.
    let dir = Scratch::new();
    let o = run(&dir, "while false; do if :; then :; fi x; done; echo end");
    assert_eq!(
        (out(&o), rc(&o)),
        ("end\n", Some(0)),
        "bash rc 2; an unrun body never parses, so it never reports: stderr={}",
        err(&o)
    );

    // **#408** landed: bash feeds the here-doc body to the block, and so does
    // fdshell now — both rows are parity (the whole matrix is in
    // `a_here_doc_or_here_string_on_a_closer_feeds_the_block`).
    let dir = Scratch::new();
    let o = run(&dir, "while :; do cat; break; done <<T\nbodyT\nT");
    assert_eq!(
        (out(&o), rc(&o)),
        ("bodyT\n", Some(0)),
        "stderr={}",
        err(&o)
    );
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

/// The same floor with the close **in the tail** (`done > f 6>&-`): the scope
/// cannot restore a saved copy the script closed, so the block fails at rc 1 with
/// the target created empty and no output — byte-identical to the group twin
/// `{ echo g; } > f 6>&-` on `master`, which is the rule this task takes from the
/// group. bash gives rc 0 with the block's output in `f` and `tail` printed; the
/// owner of the floor (a script-nameable fd 6 is the saved copy) is **#184**.
#[test]
fn a_tail_that_closes_the_saved_copy_fails_the_block() {
    let dir = Scratch::new();
    let o = run(&dir, "while :; do echo a; break; done > f 6>&-; echo tail");
    assert_eq!((out(&o), rc(&o)), ("", Some(1)), "stderr={}", err(&o));
    assert!(
        err(&o).contains("failed to close redirection target fd 6"),
        "the restore names the fd it could not close: stderr={}",
        err(&o)
    );
    assert_eq!(
        file(&dir, "f"),
        Some(b"".to_vec()),
        "the block's output never reaches the target: the scope opened and failed"
    );

    let dir = Scratch::new();
    let o = run(&dir, "{ echo g; } > f 6>&-; echo tail");
    assert_eq!(
        (out(&o), rc(&o), file(&dir, "f")),
        ("", Some(1), Some(b"".to_vec())),
        "the group twin behaves identically (already `master`'s behavior): stderr={}",
        err(&o)
    );
}

/// The here-doc half of this tail (task **#408**): a `<<`/`<<-`/`<<<` operator —
/// and its io-number form `N<<word` — written after a closing keyword belongs to
/// the **whole block**, so the block's `cat`/`read`-style body commands read the
/// claimed body stream and the shell's own fds are back when the block ends.
/// Every value below is bash 5.3.9's own output for the same script, measured
/// 2026-11-03 on this build; the rows that are **not** parity name their owner in
/// `the_here_doc_tail_rows_that_keep_their_measured_values`. The unit rows — one
/// per arm — are `parse/block_tail/heredoc/tests.rs` (integration tests
/// contribute no coverage, LESSONS).
#[test]
fn a_here_doc_or_here_string_on_a_closer_feeds_the_block() {
    for (script, want) in [
        ("while :; do cat; break; done <<T\nbody\nT", "body\n"),
        ("while :; do cat; break; done << T\nbody\nT", "body\n"),
        ("while :; do cat; break; done <<-T\n\tbody\n\tT", "body\n"),
        ("while :; do cat <&3; break; done 3<<T\nbody\nT", "body\n"),
        (
            "while read l; do echo \"[$l]\"; break; done <<T\nbody\nT",
            "[body]\n",
        ),
        (
            "while read l; do echo \"[$l]\"; break; done <<< hi",
            "[hi]\n",
        ),
        ("while :; do cat; break; done 0<<<hi", "hi\n"),
        ("while :; do cat; break; done <<< hi", "hi\n"),
        ("if true; then cat; fi <<T\nbody\nT", "body\n"),
        ("case x in\nx) cat;;\nesac <<T\nbody\nT", "body\n"),
        ("for ((i=0;i<1;i++)); do cat; done <<T\nbody\nT", "body\n"),
        // One stream for the whole block: the body prints **once** for the two
        // iterations, so the claim belongs to the block and not to a round.
        ("for x in a b; do cat; done <<T\nbody\nT", "body\n"),
        // A loop whose condition fails on the first test never runs its body, and
        // an unrun body never reads the stream (bash prints nothing).
        ("until :; do cat; break; done <<T\nbody\nT", ""),
    ] {
        let dir = Scratch::new();
        let o = run(&dir, script);
        assert_eq!(
            (out(&o), rc(&o)),
            (want, Some(0)),
            "{script:?}: stderr={}",
            err(&o)
        );
    }

    // The target file, in both operator orders: the here-doc feeds stdin, `> f`
    // takes stdout, and the body lands in `f` with stdout empty.
    for (script, want) in [
        ("while :; do cat; break; done <<T > f\nbody\nT", "body\n"),
        ("while :; do cat; break; done > f <<T\nbody\nT", "body\n"),
    ] {
        let dir = Scratch::new();
        let o = run(&dir, script);
        assert_eq!(
            (out(&o), rc(&o), file(&dir, "f")),
            ("", Some(0), Some(want.as_bytes().to_vec())),
            "{script:?}: stderr={}",
            err(&o)
        );
    }

    // The scope is undone at the block's end: the body first, then the shell's
    // own stdout answers the statement written behind the closer's separator.
    let dir = Scratch::new();
    let o = run(&dir, "while :; do cat; break; done <<T; echo tail\nbody\nT");
    assert_eq!(
        (out(&o), rc(&o)),
        ("body\ntail\n", Some(0)),
        "stderr={}",
        err(&o)
    );

    // A pipeline stage: the block's stdin is the body and the stage sees it.
    let dir = Scratch::new();
    let o = run(&dir, "while :; do cat; break; done <<T | cat\nbody\nT");
    assert_eq!((out(&o), rc(&o)), ("body\n", Some(0)), "stderr={}", err(&o));

    // The `$( … )` capture: the body reaches the substitution's stdout. bash
    // additionally warns (its delimiter search sees `T)` as the last line and the
    // stream ends), fdshell parses per statement and stays silent — the warning-
    // count divergence of task #364's rule, pinned at the other granularity there.
    let dir = Scratch::new();
    let o = run(
        &dir,
        "echo \"$(while :; do cat; break; done <<T\nbody\nT)\"",
    );
    assert_eq!((out(&o), rc(&o)), ("body\n", Some(0)), "stderr={}", err(&o));

    // The body-position operator written **before** the closer's one on the same
    // physical line owns the first region of the line's stream, so the closer
    // claims the second in global per-line operator order (LESSONS).
    let dir = Scratch::new();
    let o = run(
        &dir,
        "while :; do cat <<A; break; done <<T\nbodyA\nA\nbodyT\nT",
    );
    assert_eq!(
        (out(&o), rc(&o)),
        ("bodyA\n", Some(0)),
        "bash agrees: the closer's own claim is the second region: stderr={}",
        err(&o)
    );
}

/// The rows of the same family that keep their **measured** values, each with its
/// owner. All values measured 2026-11-03 on this build against bash 5.3.9.
#[test]
fn the_here_doc_tail_rows_that_keep_their_measured_values() {
    // The `&&` row: the block is the left side of a cond operator, so its text is
    // a run and the body stream is allocated at the **run's** granularity (task
    // #403's rule) — the claim that reaches the block's builder is empty, the
    // parse warns, and only the tail's own command prints. Predates task #408
    // (measured identically on `master` before the tail was taught to parse).
    let dir = Scratch::new();
    let o = run(
        &dir,
        "while :; do cat; break; done <<T && echo tail\nbody\nT",
    );
    assert_eq!(
        (out(&o), rc(&o)),
        ("tail\n", Some(0)),
        "bash prints the body first: stderr={}",
        err(&o)
    );
    assert!(
        err(&o).contains("delimited by end-of-file"),
        "the empty claim is the end-of-file shape: stderr={}",
        err(&o)
    );

    // A `<<` glued inside the closer's word opens no word, so the byte scan counts
    // no operator and nothing is claimed: the pinned house rule ("a `<<` glued
    // inside a word stays word data") through the keyword-glue exception. The body
    // lines stay commands, and they fail. Aligning it needs a byte-scan arm.
    let dir = Scratch::new();
    let o = run(&dir, "while :; do cat; break; done<<T\nbody\nT");
    assert_eq!(
        (out(&o), rc(&o)),
        ("", Some(1)),
        "bash prints the body: stderr={}",
        err(&o)
    );
    assert!(err(&o).contains("\"body\" not found"), "stderr={}", err(&o));

    // A multi-line block whose closer operator interleaves with a body-position
    // operator: the rc-1-vs-2 family (bash stops on the whole construct at rc 2,
    // the house convention reports at rc 1); the scan is not to be "fixed".
    let dir = Scratch::new();
    let o = run(
        &dir,
        "while :; do cat <<A; break;\ndone <<T\nbodyA\nA\nbodyT\nT",
    );
    assert_eq!(
        (out(&o), rc(&o)),
        ("", Some(1)),
        "bash: rc 2, stderr={}",
        err(&o)
    );
    assert!(err(&o).contains("expected 'done'"), "stderr={}", err(&o));

    // A **word** behind a heredoc on the closer is task #405's report. The
    // delimiter is found in the body lines below, so neither shell warns; the
    // difference is only the rc-1-vs-2 convention (the house parse-error rule).
    let dir = Scratch::new();
    let o = run(&dir, "while :; do cat; break; done <<T x\nbody\nT");
    assert_eq!(
        (out(&o), rc(&o)),
        ("", Some(1)),
        "bash: rc 2, stderr={}",
        err(&o)
    );
    assert!(
        err(&o).contains("must be followed by a separator"),
        "stderr={}",
        err(&o)
    );

    // An operator whose delimiter line never arrived is accepted with the empty
    // claim it covers, at rc 0 — task #364's rule, pinned for this tail.
    let dir = Scratch::new();
    let o = run(&dir, "while :; do cat; break; done <<T");
    assert_eq!(
        (out(&o), rc(&o)),
        ("", Some(0)),
        "bash agrees: stderr={}",
        err(&o)
    );
    assert!(
        err(&o).contains("delimited by end-of-file"),
        "stderr={}",
        err(&o)
    );

    // The `wait` round is fdshell-only (bash has no twin: it stops at its rc 2
    // syntax error on the clause), so this row pins the **decision**: the shared
    // module judges its closer's tail exactly as it judges the others — the tail
    // is taken, the body lines are claimed (they never run as commands), and the
    // round fails on the unset `%rd` fd.
    let dir = Scratch::new();
    let o = run(&dir, "wait\nreadable %rd) echo hi ;;\ndone <<T\nbody\nT");
    assert_eq!(rc(&o), Some(1), "stderr={}", err(&o));
    assert!(err(&o).contains("rd is not set"), "stderr={}", err(&o));
    assert!(
        !err(&o).contains("not found"),
        "the body lines are claimed, never commands: stderr={}",
        err(&o)
    );

    // The stdin feed asserted through a builtin that reads it: the body lands in
    // `read`, and the loop's **status** stays the pre-existing divergence —
    // fdshell reports the failing condition's status (rc 1) where bash reports the
    // last body command's (rc 0); the here-doc itself is parity.
    let dir = Scratch::new();
    let o = run(&dir, "while read l; do echo \"[$l]\"; done <<T\nbody\nT");
    assert_eq!(out(&o), "[body]\n", "bash agrees: stderr={}", err(&o));
    assert_eq!(
        rc(&o),
        Some(1),
        "bash rc 0 — the loop-status rule: stderr={}",
        err(&o)
    );
}
