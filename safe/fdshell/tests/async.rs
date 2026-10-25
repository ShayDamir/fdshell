#![cfg_attr(test, allow(clippy::unwrap_used))]

//! The plain `&` async operator (task #326): the statement before it becomes
//! a background job (the shell forks itself, the child runs the identical
//! foreground path), the parent continues with status 0. Every async pin ends
//! with `wait` so the harness's stdout pipe closes deterministically
//! (background children inherit stdout, LESSONS), and asserts output *sets*
//! for async output — parent and child interleave, order is never pinned.

use std::process::Command;
use std::process::id;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> std::process::Output {
    Command::new(BIN).args(["-c", script]).output().unwrap()
}

/// The output lines as a set (async output has no order).
fn lines(out: &std::process::Output) -> Vec<String> {
    let stdout = str::from_utf8(&out.stdout).unwrap();
    let mut v: Vec<String> = stdout.lines().map(str::to_string).collect();
    v.sort();
    v
}

/// A per-test scratch file name (background children write their own stdout
/// redirect, so a multi-job pin never depends on interleaving).
fn scratch(tag: &str) -> String {
    format!("async-{tag}-{}", id())
}

fn contents(path: &str) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let _ = std::fs::remove_file(path);
    text
}

fn rc(out: &std::process::Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

/// Run with the shell's fd 3 **closed from the start**: `sh` closes it before
/// `exec`-ing fdshell, so the fd 3 the child (and every background child it
/// forks) inherits is deterministic whatever the harness held. A pin that
/// asserts fd 3 is *closed* goes through this helper — `cargo-mutants` hands
/// every test process its jobserver pipes as open fds ≥ 3, which would flip
/// those outcomes and break the unmutated baseline (LESSONS, task #276).
/// stdin is `/dev/null` too: a backgrounded `cat` inherits it, and a stdin
/// that never reaches EOF would keep the pin's own `wait` blocked.
fn run_fd3_closed(script: &str) -> (String, String, i32) {
    let output = Command::new("sh")
        .args(["-c", "exec 3>&-; exec \"$0\" -c \"$1\"", BIN, script])
        .stdin(std::process::Stdio::null())
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

fn contains_all(out: &std::process::Output, want: &[&str]) {
    let stdout = str::from_utf8(&out.stdout).unwrap().to_string();
    for w in want {
        assert!(stdout.contains(w), "want {w} in stdout={stdout}");
    }
}

/// The number behind the first `pid=` in stdout, read digit-by-digit: the
/// background child shares the pipe, so its line may land *before* the
/// parent's, and only the digits of the parent's own line are the pid.
fn reported_pid(out: &std::process::Output) -> i64 {
    let stdout = str::from_utf8(&out.stdout).unwrap().to_string();
    let tail = stdout.split("pid=").nth(1).unwrap_or_default();
    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().unwrap_or(-1)
}

/// `echo hi &` is a background job: it runs, the shell continues, `wait` reaps
/// it, and the script's status is 0.
#[test]
fn async_statement_runs_and_is_reaped_by_wait() {
    let out = run("builtin echo hi &\nwait");
    assert_eq!(rc(&out), 0);
    assert_eq!(lines(&out), ["hi"]);
}

/// A shell function with a trailing `&` runs **in the forked child** (bash
/// parity, measured identical: `in`, rc 0) — the child's copy of the state
/// carries the function table the parent defined, so the async job is the
/// whole command, not an external lookup.
#[test]
fn function_in_background_runs_in_the_child() {
    let out = run("f(){ builtin echo in; }; f &\nwait");
    assert_eq!(rc(&out), 0);
    assert_eq!(lines(&out), ["in"]);
}

/// Two `&` on one line are two jobs (bash parity): both run, `wait` reaps both.
#[test]
fn two_ampersands_are_two_jobs() {
    let (a, b) = (scratch("2a"), scratch("2b"));
    let out = run(&format!(
        "builtin echo a >{a} & builtin echo b >{b} &\nwait"
    ));
    let got = (contents(&a), contents(&b));
    assert_eq!(rc(&out), 0);
    assert_eq!(got, (String::from("a\n"), String::from("b\n")));
}

/// The parent does not wait: the statement after the `&` runs alongside.
#[test]
fn parent_continues_without_waiting() {
    let out = run("builtin echo bg & builtin echo fg");
    assert_eq!(rc(&out), 0);
    contains_all(&out, &["bg", "fg"]);
}

/// POSIX 2.14: `wait` with no operands returns 0 whatever the reaped jobs'
/// statuses were (measured bash 5.3.9: `false & wait; echo $?` → `0`).
#[test]
fn wait_without_operands_returns_zero() {
    let out = run("builtin false &\nwait; builtin echo rc=$?");
    assert_eq!(rc(&out), 0);
    contains_all(&out, &["rc=0"]);
}

/// `wait <pid>` on a nameless job reaps it and reports the child's status.
#[test]
fn wait_by_pid_reports_the_failing_child() {
    let out = run("builtin false &\nwait $!; builtin echo rc=$?");
    assert_eq!(rc(&out), 0);
    contains_all(&out, &["rc=1"]);
}

/// `$!` reports the last background job's pid: numeric and non-zero (a
/// backgrounded pipeline's `$!` is the job's shell pid, not the last stage's —
/// measured divergence, so only "numeric and > 0" is pinned).
#[test]
fn last_bg_pid_is_reported_numerically() {
    let out = run("builtin true &\nbuiltin echo pid=$!; wait");
    assert_eq!(rc(&out), 0);
    assert!(
        reported_pid(&out) > 0,
        "stdout={:?}",
        str::from_utf8(&out.stdout)
    );
}

/// A pipeline with a trailing `&` is one job, reaped by one `wait`, and `$!`
/// reports the job's pid (numeric, > 0 — for a pipeline it is the job's shell
/// pid, not the last stage's: measured divergence, so only "numeric" pins).
#[test]
fn pipeline_is_one_async_job() {
    let out = run("builtin echo a | builtin echo b &\nbuiltin echo pid=$!; wait");
    assert_eq!(rc(&out), 0);
    contains_all(&out, &["b"]);
    assert!(
        reported_pid(&out) > 0,
        "stdout={:?}",
        str::from_utf8(&out.stdout)
    );
}

/// `echo a && echo b &` is two async jobs in fdshell where bash backgrounds the
/// whole list as one (measured divergence, owner #352); the output set and rc
/// are identical.
#[test]
fn and_list_with_trailing_ampersand_runs_both_elements() {
    let (a, b) = (scratch("ala"), scratch("alb"));
    let out = run(&format!(
        "builtin echo a >{a} && builtin echo b >{b} &\nwait"
    ));
    let got = (contents(&a), contents(&b));
    assert_eq!(rc(&out), 0);
    assert_eq!(got, (String::from("a\n"), String::from("b\n")));
}

/// A top-level compound with a trailing `&` runs **foreground** in fdshell
/// (bash backgrounds the whole compound as one job; measured divergence,
/// owner #352): `x` prints before `y`, both print, rc 0.
#[test]
fn compound_with_trailing_ampersand_runs_foreground() {
    let out = run("if :; then builtin echo x; fi & builtin echo y");
    assert_eq!(rc(&out), 0);
    let stdout = str::from_utf8(&out.stdout).unwrap().to_string();
    assert_eq!(
        (stdout.find('x'), stdout.find('y')),
        (Some(0), Some(2)),
        "stdout={stdout}"
    );
}

/// An assignment-only statement with a trailing `&` runs foreground in fdshell
/// (bash runs it in the async child, so `x` stays unset; measured divergence,
/// owner #353).
#[test]
fn assignment_only_statement_runs_foreground() {
    let out = run("x=1 & builtin echo v=$x; wait");
    assert_eq!(rc(&out), 0);
    contains_all(&out, &["v=1"]);
}

/// bash's preflight property: an invalid split runs **nothing** and exits 1 —
/// validated at the split, before any statement executes (`script.rs`'s
/// pre-pass).
#[test]
fn invalid_splits_run_nothing() {
    for script in [
        "builtin echo a & ; builtin echo b",
        "builtin echo a & | builtin cat",
        "builtin echo a & & builtin b",
        "builtin echo a ; & builtin b",
        "& builtin echo b",
    ] {
        let out = run(script);
        assert_eq!(rc(&out), 1, "script={script}");
        assert_eq!(str::from_utf8(&out.stdout).unwrap(), "", "script={script}");
        assert!(
            str::from_utf8(&out.stderr)
                .unwrap()
                .contains("unexpected character"),
            "script={script} stderr={:?}",
            str::from_utf8(&out.stderr)
        );
    }
}

/// A `&` glued to a word character is fdshell's task-name word (the
/// `&>&name`/`waitpid &name` convention), never the async operator.
#[test]
fn ampersand_glued_to_a_word_is_name_data() {
    let out = run("waitpid &nope");
    assert_eq!(rc(&out), 1);
    assert!(
        str::from_utf8(&out.stderr)
            .unwrap()
            .contains("task not found"),
        "stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// The named background-capture form (`&>&name`) is unchanged by the plain `&`:
/// the task is stored by name and reaped by name.
#[test]
fn named_background_capture_still_works() {
    let out = run("builtin echo hi &>&j; waitpid &j; builtin echo done");
    assert_eq!(rc(&out), 0);
    contains_all(&out, &["hi", "done"]);
}

/// `>&` is one operator token in bash across spaces (`cat > &f` is a redirect,
/// never an async split): fdshell keeps its "invalid redirect" error.
#[test]
fn redirect_operator_owns_a_later_ampersand() {
    let out = run("cat > &f");
    assert_eq!(rc(&out), 1);
    assert!(
        str::from_utf8(&out.stderr)
            .unwrap()
            .contains("invalid redirect"),
        "stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// A here-doc line with a trailing `&` works (the #313 parse error is closed):
/// the child prints its body, `wait` reaps it.
#[test]
fn here_doc_with_trailing_ampersand_prints_its_body() {
    let out = run("cat <<EOF &\nbody\nEOF\nwait");
    assert_eq!(rc(&out), 0);
    assert_eq!(lines(&out), ["body"]);
}

/// A here-string on fd 3 with a trailing `&` runs the job at rc 0 and prints
/// nothing — bash parity (`cat 3<<<x` puts the text on fd 3, `cat` reads
/// stdin; measured identical in both, with and without the `&`). The #313
/// family row in `POSIX_COMPATIBILITY.md` (a parse error) is retired. Run
/// through [`run_fd3_closed`]: the outcome is fd-3-state-dependent (an
/// inherited open fd 3 would change what the child's `cat` inherits) and its
/// stdin is the harness's, so the run is made deterministic at both.
#[test]
fn here_string_on_fd3_with_trailing_ampersand_matches_bash() {
    let (out, _err, code) = run_fd3_closed("cat 3<<<x &\nwait");
    assert_eq!(code, 0);
    assert!(out.lines().next().is_none(), "want no stdout, got {out:?}");
}

/// Quoting and escaping own the `&`: a quoted/escaped `&` is word data, never
/// a control operator.
#[test]
fn quoted_and_escaped_ampersand_stay_word_data() {
    for (script, want) in [("builtin echo \"&\"", "&"), ("builtin echo a \\&", "a &")] {
        let out = run(script);
        assert_eq!(rc(&out), 0, "script={script}");
        assert_eq!(
            str::from_utf8(&out.stdout).unwrap().trim(),
            want,
            "script={script}"
        );
    }
}

/// Arithmetic and parenthesised `&` are untouched: inside `$((…))` and `$( … )`
/// the byte is data (the scan's bare-gate excludes it).
#[test]
fn ampersand_inside_expansions_is_untouched() {
    let out = run("builtin echo $((6 & 3)); builtin echo $(builtin echo x)");
    assert_eq!(rc(&out), 0);
    assert_eq!(lines(&out), ["2", "x"]);
}

/// A `&` glued to a preceding operator byte is that operator, not async:
/// `&&`'s second byte, `N>&file`, `|&`, `<&`, `>&`-shaped words.
#[test]
fn ampersand_glued_to_an_operator_is_not_async() {
    // `&&`'s second byte is the AND operator: `and` prints, rc 0.
    let out = run("builtin true && builtin echo and");
    assert_eq!(rc(&out), 0);
    assert_eq!(lines(&out), ["and"]);
    // `N>&N` is the fd-dup operator: fd 3 is closed, so the *redirect* fails
    // (an async `&` here would have launched `echo` in the background). Run
    // through [`run_fd3_closed`]: the assertion is about the inherited fd 3.
    let (_out, err, code) = run_fd3_closed("builtin echo three 1>&3");
    assert_eq!(code, 1);
    assert!(err.contains("fd 3 is not open"), "{err}");
}

/// `cd x &` (a parent-side builtin in the discarded child): the child's `cd`
/// never reaches the parent (measured bash parity).
#[test]
fn cd_in_background_leaves_the_parent_cwd() {
    let cwd = std::env::current_dir().unwrap();
    let out = run("cd /tmp & builtin pwd");
    assert_eq!(rc(&out), 0);
    assert_eq!(
        str::from_utf8(&out.stdout).unwrap().trim(),
        cwd.to_string_lossy().trim_end_matches('/'),
        "stdout={:?}",
        str::from_utf8(&out.stdout)
    );
}

/// `exit &` terminates the child only (the child's copy of the shell): the
/// parent keeps running with status 0.
#[test]
fn exit_in_background_leaves_the_parent_alive() {
    let out = run("exit & builtin echo alive; wait");
    assert_eq!(rc(&out), 0);
    contains_all(&out, &["alive"]);
}

/// `time cmd &` is the same family as the compound divergence: `time` is not a
/// `Cmd`/`Pipeline`, so its statement runs foreground (owner #352).
#[test]
fn time_with_trailing_ampersand_runs_foreground() {
    let out = run("time builtin echo t & builtin echo after");
    assert_eq!(rc(&out), 0);
    contains_all(&out, &["t", "after"]);
}

/// A capture on a backgrounded command commits in the **discarded child**: the
/// parent never sees the fd variable (measured divergence; the named capture
/// form for background stays `&>&name`).
#[test]
fn capture_of_a_backgrounded_command_stays_in_the_child() {
    let name = format!("async-capture-{}", id());
    let out = run(&format!(
        "builtin openat2 --flags O_CREAT --flags O_EXCL --flags O_RDWR --mode 0644 {name} %>%{name} &\nwait; builtin echo v=%{name}; builtin echo done"
    ));
    let _ = std::fs::remove_file(&name);
    assert_eq!(rc(&out), 0);
    let tail = format!("v=%{name}");
    contains_all(&out, &[&tail, "done"]);
}

/// The multi-line streaming shape (`echo a &\n; echo b`): line 1 executes and
/// the `;` line is fdshell's accepted empty-statement divergence (bash stops
/// with rc 2 and runs only line 1; the same-line `& ;` form matches bash and
/// is pinned by [`invalid_splits_run_nothing`]).
#[test]
fn multi_line_ampersand_then_semicolon_streams() {
    let (a, b) = (scratch("sla"), scratch("slb"));
    let out = run(&format!("builtin echo a >{a} &\n; builtin echo b >{b}"));
    let got = (contents(&a), contents(&b));
    assert_eq!(rc(&out), 0);
    assert_eq!(got, (String::from("a\n"), String::from("b\n")));
}

/// A `&` whose next line opens with `&` claims the statement above it
/// (the deferral): `echo a &\n& echo b` is `echo a` async twice over, both
/// print, `wait` reaps, rc 0.
#[test]
fn ampersand_on_the_next_line_claims_the_statement_above_it() {
    let (a, b) = (scratch("mla"), scratch("mlb"));
    let out = run(&format!(
        "builtin echo a >{a} &\n& builtin echo b >{b}\nwait"
    ));
    let got = (contents(&a), contents(&b));
    assert_eq!(rc(&out), 0);
    assert_eq!(got, (String::from("a\n"), String::from("b\n")));
}

/// The deferral claims the statement above a `&` that opens its line **after
/// indentation**: the newline-to-`&` gap is spaces and tabs, so the claim
/// depends on `bg::async_ahead` skipping exactly the blanks between them.
/// Both statements run, `wait` reaps, rc 0 (bash: `a` then rc 2 — the
/// deferral divergence documented in POSIX_COMPATIBILITY).
#[test]
fn indented_ampersand_on_the_next_line_claims_the_statement_above_it() {
    for indent in [" ", "\t", " \t "] {
        let (a, b) = (scratch("mla"), scratch("mlb"));
        let out = run(&format!(
            "builtin echo a >{a}\n{indent}& builtin echo b >{b}\nwait"
        ));
        let got = (contents(&a), contents(&b));
        assert_eq!(
            rc(&out),
            0,
            "indent {indent:?}: {:?}",
            str::from_utf8(&out.stderr)
        );
        assert_eq!(
            got,
            (String::from("a\n"), String::from("b\n")),
            "indent {indent:?}"
        );
    }
}

/// No stray background child survives the script: the parent's own `wait`
/// reaps every job, so the harness's pipe closes (hermeticity).
#[test]
fn jobs_do_not_outlive_the_script() {
    let (a, b, c) = (scratch("j1"), scratch("j2"), scratch("j3"));
    let out = run(&format!(
        "builtin echo one >{a} & builtin echo two >{b} & builtin echo three >{c} &\nwait"
    ));
    let got = [contents(&a), contents(&b), contents(&c)];
    assert_eq!(rc(&out), 0);
    assert_eq!(
        got,
        [
            String::from("one\n"),
            String::from("two\n"),
            String::from("three\n")
        ]
    );
}

// --- a plain `&` inside a block's text (task #354) ---

/// The shapes of the block family: the `&` is written **inside** a single-line
/// block whose closing keyword follows it on the same line (the output file is
/// `{f}`), and the statement must run as a background job **inside the block
/// body** (bash 5.3.9 parity, measured). The multi-line shapes ride along: the
/// body line is its own statement there, and the two paths must not diverge.
fn block_shapes() -> Vec<String> {
    vec![
        "for i in 1; do builtin echo hi >{f} & done".into(),
        "{ builtin echo hi >{f} & }".into(),
        "f(){ builtin echo hi >{f} & }; f".into(),
        "function f { builtin echo hi >{f} & }; f".into(),
        "function f() { builtin echo hi >{f} & }; f".into(),
        "case x in x) builtin echo hi >{f} & ;; esac".into(),
        "case x in x) builtin echo hi >{f} & esac".into(),
        "case x in x) builtin echo hi >{f} &;; esac".into(),
        "case x in x) builtin echo hi >{f} & ;;esac".into(),
        "case x in x) builtin true && builtin echo hi >{f} & ;; esac".into(),
        "true && { builtin echo hi >{f} & }".into(),
        "while :; do builtin echo hi >{f} & break; done".into(),
        "if true; then builtin echo hi >{f} & fi".into(),
        "case x in x) builtin echo hi >{f} &\n;;\nesac".into(),
    ]
}

/// Every block shape runs its `&`-terminated statement as a job: the file
/// appears, `wait` reaps the job, rc 0.
#[test]
fn every_block_shape_backgrounds_its_ampersand_statement() {
    for shape in block_shapes() {
        let f = scratch("blkshape");
        let script = shape.replace("{f}", &f);
        let out = run(&format!("{script}\nwait"));
        assert_eq!(
            rc(&out),
            0,
            "shape {shape:?}: {:?}",
            str::from_utf8(&out.stderr)
        );
        assert_eq!(contents(&f), "hi\n", "shape {shape:?}");
    }
}

/// The `& ;` **inside** a block body runs nothing (task #355): the body slice
/// keeps the glued `;`, the body's own scan marks the split invalid, and the
/// preflight bails before anything executes — rc 1, empty stdout, the
/// redirect target **absent** (the job never forked; LESSONS: pin at the
/// filesystem), stderr the scanner's `unexpected character` (bash rc 2).
#[test]
fn invalid_block_bodies_run_nothing() {
    for shape in [
        "for i in 1; do builtin echo a >{f} & ; done",
        "if true; then builtin echo a >{f} & ; fi",
        "{ builtin echo a >{f} & ; }",
        "f(){ builtin echo a >{f} & ; }; f",
        "case x in x) builtin echo a >{f} & ; ;; esac",
    ] {
        let f = scratch("invalblk");
        let script = shape.replace("{f}", &f);
        let out = run(&script);
        assert_eq!(
            rc(&out),
            1,
            "shape {shape:?}: {:?}",
            str::from_utf8(&out.stderr)
        );
        assert_eq!(str::from_utf8(&out.stdout).unwrap(), "", "shape {shape:?}");
        assert!(
            str::from_utf8(&out.stderr)
                .unwrap()
                .contains("unexpected character"),
            "shape {shape:?}: {:?}",
            str::from_utf8(&out.stderr)
        );
        assert!(
            !std::path::Path::new(&f).exists(),
            "shape {shape:?}: the job ran"
        );
        let _ = std::fs::remove_file(&f);
    }
}

/// A `for` body with two `&` is two jobs, and so is a group's body and a `case`
/// arm's: every iteration/arm runs both statements in the background.
#[test]
fn two_ampersands_in_one_block_body_are_two_jobs() {
    for shape in [
        "for i in 1; do builtin echo a >{a} & builtin echo b >{b} & done",
        "{ builtin echo a >{a} & builtin echo b >{b} & }",
        "case x in x) builtin echo a >{a} & builtin echo b >{b} & ;; esac",
        "case x in x) builtin echo a >{a} & builtin echo b >{b} & esac",
    ] {
        let (a, b) = (scratch("blk2a"), scratch("blk2b"));
        let script = shape.replace("{a}", &a).replace("{b}", &b);
        let out = run(&format!("{script}\nwait"));
        let got = (contents(&a), contents(&b));
        assert_eq!(
            rc(&out),
            0,
            "shape {shape:?}: {:?}",
            str::from_utf8(&out.stderr)
        );
        assert_eq!(
            got,
            (String::from("a\n"), String::from("b\n")),
            "shape {shape:?}"
        );
    }
}

/// A `for` loop runs the body's job on **every** iteration: two iterations
/// append twice, and one `wait` reaps both. (The count pins the iteration: a
/// `>` target carrying `$i` is fdshell's own expansion divergence, so the file
/// name stays fixed here.)
#[test]
fn for_loop_runs_the_body_job_every_iteration() {
    let s = scratch("forbody");
    let out = run(&format!(
        "for i in 1 2; do builtin echo hi >>{s} & done\nwait"
    ));
    let body = contents(&s);
    assert_eq!(rc(&out), 0);
    assert_eq!(body.lines().count(), 2, "body={body:?}");
}

/// The block body's job is a real job record: `$!` reports its pid numerically
/// (the pins the "the `&` before a closer is eaten" mutant class — the body
/// text keeps the `&`).
#[test]
fn block_body_job_reports_its_pid() {
    for shape in [
        "for i in 1; do builtin true & done",
        "{ builtin true & }",
        "case x in x) builtin true & ;; esac",
        "if true; then builtin true & fi",
        "f(){ builtin true & }; f",
    ] {
        let out = run(&format!("{shape}\nbuiltin echo pid=$!; wait"));
        assert_eq!(rc(&out), 0, "shape {shape:?}");
        assert!(
            reported_pid(&out) > 0,
            "shape {shape:?}: stdout={}",
            str::from_utf8(&out.stdout).unwrap_or_default()
        );
    }
}

/// An `if` body whose `then` and `else` arms both end in `&`: the taken arm's
/// statement is a job, the untaken arm is not run at all (bash parity).
#[test]
fn if_arms_background_their_own_statements() {
    let (t, e) = (scratch("ifthen"), scratch("ifelse"));
    let script = format!("if true; then builtin echo a >{t} & else builtin echo b >{e} & fi\nwait");
    let out = run(&script);
    let (taken, untaken) = (contents(&t), contents(&e));
    assert_eq!(rc(&out), 0);
    assert_eq!(taken, "a\n");
    assert_eq!(untaken, "");
}

/// A block body with a `<<` operator and a trailing `&` keeps its heredoc body
/// (the append-mode offset moves with the `&` token, task #354's risk R5): the
/// empty line prints, rc 0, as bash. The `case` and `for` shapes are measured
/// identical; the group shape stays #313's seam.
#[test]
fn heredoc_in_a_block_body_with_a_trailing_ampersand() {
    for shape in [
        "for i in 1; do builtin echo <<T & done",
        "case x in x) builtin echo <<T & ;; esac",
        "if true; then builtin echo <<T & fi",
    ] {
        let out = run(&format!("{shape}\nT\nwait"));
        assert_eq!(rc(&out), 0, "shape {shape:?}");
        assert_eq!(
            lines(&out),
            [String::new()],
            "shape {shape:?}: stdout={}",
            str::from_utf8(&out.stdout).unwrap_or_default()
        );
    }
}

/// The `& ;;` peek guard stays closed to a `case` arm: a `;;` **outside** a
/// block (`echo a & ;; echo b`) is bash's syntax error, rc 2, fdshell rc 1 —
/// the relaxation `split.rs::peek` must never get.
#[test]
fn a_semi_semi_outside_a_block_stays_an_invalid_split() {
    for script in [
        "builtin echo a & ;; builtin echo b",
        "builtin echo a & ; builtin echo b",
        "builtin echo a & & builtin echo b",
    ] {
        let out = run(script);
        assert_eq!(rc(&out), 1, "script {script:?}");
        assert!(
            str::from_utf8(&out.stderr)
                .unwrap_or_default()
                .contains("unexpected character in this context"),
            "script {script:?}"
        );
    }
    // The `case` twin: the opener gate owns the run, and the block the `&`
    // leaves behind is unclosed, so the script-level parse error is the report
    // (bash rc 2, fdshell rc 1 — the documented rc-1-vs-2 family).
    let out = run("case x in x) builtin echo a & ;; builtin echo b");
    assert_eq!(rc(&out), 1);
    assert!(
        str::from_utf8(&out.stderr)
            .unwrap_or_default()
            .contains("parse error"),
        "stderr={}",
        str::from_utf8(&out.stderr).unwrap_or_default()
    );
}

/// Task #313: a here-doc statement written **behind** a plain `&` on the same
/// physical line is a statement of its own, with its own body region. bash
/// backgrounds the prefix and runs the suffix in the foreground; the measured
/// pair (both bodies, rc 0) is now identical. Before the fix the suffix's body
/// lines ran as statements (`"y" not found`, `"B" not found`): the flush the
/// `&` performed parked the cursor past the whole line's body regions, so the
/// suffix command text was never scanned.
///
/// Every row ends with `wait` as its **last** line: a bare `wait` at the end of
/// a line inside a multi-line script opens fdshell's event-`wait` block
/// (`POSIX_COMPATIBILITY.md` "`wait` is also a block keyword"), and the reap is
/// what closes the harness's stdout pipe. Output is compared as a **set**:
/// parent and child interleave, order is never pinned (LESSONS).
#[test]
fn a_heredoc_behind_an_ampersand_prints_both_bodies() {
    for (shape, want) in [
        ("cat <<A & cat <<B\nx\nA\ny\nB", vec!["x", "y"]),
        ("cat <<A & cat <<'B'\nx\nA\ny\nB", vec!["x", "y"]),
        ("cat <<A & cat <<-B\nx\nA\n\ty\n\tB", vec!["x", "y"]),
        ("cat <<A & cat <<B | wc -l\nx\nA\ny\nB", vec!["1", "x"]),
        (
            "cat <<A & cat <<B & cat <<C\nx\nA\ny\nB\nz\nC",
            vec!["x", "y", "z"],
        ),
        ("cat <<A & cat <<B\t\nx\nA\ny\nB", vec!["x", "y"]),
        ("cat <<A   &   cat <<B\nx\nA\ny\nB", vec!["x", "y"]),
    ] {
        let out = run(&format!("{shape}\nwait"));
        assert_eq!(
            rc(&out),
            0,
            "shape {shape:?}: {:?}",
            str::from_utf8(&out.stderr)
        );
        assert_eq!(
            lines(&out),
            want,
            "shape {shape:?}: stdout={}",
            str::from_utf8(&out.stdout).unwrap_or_default()
        );
    }
}

/// The io-number twin (`cat 3<<B` behind the `&`, the form task #276 measures):
/// the suffix body goes to the descriptor, so stdout carries the prefix's body
/// only. The fd 3 state goes through `run_fd3_closed` (LESSONS, task #312).
#[test]
fn a_heredoc_on_fd3_behind_an_ampersand_writes_to_the_descriptor() {
    let (out, err, code) = run_fd3_closed("cat <<A & cat 3<<B\nx\nA\ny\nB\nexec 3>&-\nwait");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.lines().collect::<Vec<_>>(), ["x"], "stdout={out}");
}

/// A statement written after the pair runs once the bodies are reaped: all
/// three lines land, rc 0 (measured identical to bash). `wait;` is the
/// statement form — a bare `wait` at the line end opens the event-`wait` block.
#[test]
fn a_statement_follows_the_heredoc_pair_behind_an_ampersand() {
    let out = run("cat <<A & cat <<B\nx\nA\ny\nB\nwait;\nbuiltin echo tail");
    assert_eq!(rc(&out), 0);
    assert_eq!(lines(&out), ["tail", "x", "y"]);
}

/// The async **ordering** rule for a here-doc job: the backgrounded body's
/// stdout has no order relative to the parent's, so the job's stdout goes to a
/// file (LESSONS: a background-job pin pins the job record, never the output).
/// bash writes the same bytes to the same file.
#[test]
fn the_backgrounded_heredoc_writes_its_body_to_its_own_file() {
    let s = scratch("bgdoc");
    let out = run(&format!("cat <<T >{s} &\nbody\nT\nwait"));
    let body = contents(&s);
    assert_eq!(rc(&out), 0, "stderr={:?}", str::from_utf8(&out.stderr));
    assert_eq!(body, "body\n");
}

/// `$!` reports a here-doc-carrying async job numerically and `wait` reaps it
/// (the acceptance phrase "the background job is reaped"; fdshell, like
/// `bash -c`, never joins background jobs at shell exit — reaping is `wait`'s).
#[test]
fn a_backgrounded_heredoc_reports_and_reaps_its_job() {
    let out = run("cat <<T &\nbody\nT\nbuiltin echo pid=$!; wait");
    assert_eq!(rc(&out), 0);
    assert!(
        reported_pid(&out) > 0,
        "stdout={}",
        str::from_utf8(&out.stdout).unwrap_or_default()
    );
    contains_all(&out, &["body"]);
}

/// The block-position twins: a plain `&` **inside a block's text** terminates
/// its statement (task #354) and the here-doc statement behind it on the same
/// line is a new statement of the same body, owning its own region. All five
/// closers measured identical to bash: both bodies, rc 0.
#[test]
fn a_heredoc_behind_an_ampersand_inside_a_block_prints_both_bodies() {
    for shape in [
        "if true; then cat <<A & cat <<B\nx\nA\ny\nB\nfi",
        "{ cat <<A & cat <<B\nx\nA\ny\nB\n}",
        "f(){ cat <<A & cat <<B\nx\nA\ny\nB\n}\nf",
        "for i in 1; do cat <<A & cat <<B\nx\nA\ny\nB\ndone",
        "case x in x) cat <<A & cat <<B\nx\nA\ny\nB\n;;\nesac",
    ] {
        let out = run(shape);
        assert_eq!(
            rc(&out),
            0,
            "shape {shape:?}: {:?}",
            str::from_utf8(&out.stderr)
        );
        assert_eq!(
            lines(&out),
            ["x", "y"],
            "shape {shape:?}: stdout={}",
            str::from_utf8(&out.stdout).unwrap_or_default()
        );
    }
}

/// The **single-line group** twin of the same shape: a `&`-terminated here-doc in
/// a `{ …; }` body is a job of the group body, and its region rides with the job
/// (task **#315** closed the group body's region gap, so the group parses at all).
/// `cat` reads the region in the job and prints it, rc 0, exactly as bash.
#[test]
fn a_heredoc_behind_an_ampersand_inside_a_single_line_group_body() {
    let shape = "{ cat <<T & }\nbody\nT";
    let out = run(shape);
    assert_eq!(rc(&out), 0, "shape {shape:?}");
    assert_eq!(
        lines(&out),
        vec!["body".to_string()],
        "shape {shape:?}: stdout={}",
        str::from_utf8(&out.stdout).unwrap_or_default()
    );
}

// ─── Task #356: a `&`-terminated block **condition** is a background job ───
//
// POSIX 2.9/2.14 read a block condition as a compound list, whose terminators
// include the plain `&`. `trim_condition` cuts that `&` from the condition text,
// so the parsed block's `cond_background` flag is the fact's single owner and
// `run_cond_list`'s `background` argument makes the job (one per condition run,
// so a loop records one per iteration). Every pin ends with `wait`; async output
// is compared as a **set**, never as an order (LESSONS).

/// The acceptance shapes record one nameless job each: `$!` is a numeric pid for
/// the `if`, the `while`, the `until`, the `elif` arm, and the `&`-newline and
/// blank-line forms (bash reads `&` newline `then` exactly as `& then`).
#[test]
fn a_conditional_terminated_by_ampersand_records_one_job() {
    for shape in [
        "if true & then builtin true; fi",
        "while : & do break; done",
        "until : & do break; done",
        "if false; then :; elif true & then builtin true; fi",
        "if true &\nthen builtin true; fi",
        "if true &\n\nthen builtin true; fi",
    ] {
        let out = run(&format!("{shape}\nbuiltin echo pid=$!; wait"));
        assert_eq!(rc(&out), 0, "shape {shape:?}");
        assert!(
            reported_pid(&out) > 0,
            "shape {shape:?}: no job recorded, stdout={}",
            str::from_utf8(&out.stdout).unwrap_or_default()
        );
    }
}

/// The arm/loop that a `&`-terminated condition gates sees the **async status
/// 0** (bash parity, measured): `if false & then echo t; else echo f; fi` takes
/// the `then` arm, `while false &` runs its body once, and an `until` stops at
/// once — where master gated on the condition's own status.
#[test]
fn a_conditional_terminates_the_arm_it_gates() {
    let out = run("if false & then builtin echo t; else builtin echo f; fi\nwait");
    assert_eq!(rc(&out), 0);
    assert_eq!(lines(&out), ["t"]);
    let out = run("while false & do builtin echo x; break; done\nwait");
    assert_eq!(rc(&out), 0);
    assert_eq!(lines(&out), ["x"]);
    let out = run("until false & do builtin echo x; break; done\nwait");
    assert_eq!(rc(&out), 0);
    assert_eq!(lines(&out), Vec::<String>::new());
}

/// The condition's side effects die with the child: a `cd` written in a
/// `&`-terminated condition never moves the parent's cwd (bash parity; master's
/// foreground condition moved it).
#[test]
fn a_conditional_job_leaves_the_parent_cwd() {
    let cwd = std::env::current_dir().unwrap();
    let out = run("if cd /tmp & then builtin true; fi\nbuiltin pwd\nwait");
    assert_eq!(rc(&out), 0);
    assert_eq!(
        str::from_utf8(&out.stdout).unwrap().trim(),
        cwd.to_string_lossy().trim_end_matches('/'),
        "stdout={:?}",
        str::from_utf8(&out.stdout)
    );
}

/// `exit` in a `&`-terminated condition dies with the child: the script keeps
/// running and takes the arm (bash prints `x` then `tail`).
#[test]
fn exit_in_a_conditional_leaves_the_script_running() {
    let out = run("if exit & then builtin echo x; fi\nbuiltin echo tail\nwait");
    assert_eq!(rc(&out), 0);
    contains_all(&out, &["x", "tail"]);
}

/// Pinned divergence (owner **#362**, measured identical on master): a
/// loop-control **keyword** with the `&` is not a `Cmd`/`Pipeline`, so
/// `run_parsed` runs it in the parent — the `break` ends the loop before its
/// body ever runs, where bash (the whole condition is the child) prints `x`
/// then `tail`. Pinned as measured, bounded by the `break` in the body.
#[test]
fn break_in_a_conditional_ends_the_loop_here() {
    let out = run("while break & do builtin echo x; break; done\nbuiltin echo tail\nwait");
    assert_eq!(rc(&out), 0);
    assert_eq!(lines(&out), ["tail"], "bash prints x too: #362");
}

/// A here-doc carried by a `&`-terminated condition keeps its body region (the
/// condition's bodies travel as `run_cond_list`'s `bodies`; the flag changes no
/// slice) and records its job: the body prints from the child, the arm prints
/// in the parent, `$!` is the job. Async output has no order, so the lines are
/// a set.
#[test]
fn a_conditional_with_a_heredoc_prints_its_body_and_records_a_job() {
    let out = run("if cat <<A & then builtin echo x; fi\nbody\nA\nbuiltin echo pid=$!; wait");
    assert_eq!(rc(&out), 0);
    assert!(
        reported_pid(&out) > 0,
        "stdout={}",
        str::from_utf8(&out.stdout).unwrap_or_default()
    );
    let mut got: Vec<String> = lines(&out)
        .into_iter()
        .filter(|l| !l.starts_with("pid="))
        .collect();
    got.sort();
    assert_eq!(got, ["body", "x"]);
}

/// One job **per condition run**: a three-iteration loop records three jobs, so
/// the scratch file's line count is the iteration pin and `$!` reports the last
/// job (row 19).
#[test]
fn a_conditional_records_one_job_per_iteration() {
    let f = scratch("cond-iter");
    let out = run(&format!(
        "i=0; while : & do i=$((i+1)); builtin echo $i >>{f}; [ $i -ge 3 ] && break; done\n\
         builtin echo pid=$!; wait"
    ));
    assert_eq!(rc(&out), 0);
    assert_eq!(contents(&f), "1\n2\n3\n");
    assert!(reported_pid(&out) > 0, "$! is the last job");
}

/// The residual divergences of the family, pinned as measured with their owners
/// (all measured against bash 5.3.9 the same way as the rows above):
///
/// * a **compound** condition runs foreground — `run_parsed` backgrounds only
///   `Cmd`/`Pipeline`, so `{ … } &` / `for … done &` as a condition records no
///   job and prints in the parent (owner **#352**);
/// * a condition whose last part is **short-circuited away** records no job and
///   gates on that part's status, where bash backgrounds the whole AND-list
///   (owner **#352**);
/// * a `&` on its **own line** behind the condition is the deferral family
///   (owner **#313**): fdshell accepts it and, since the slice ends in the `&`,
///   records the job;
/// * an **empty** last part of the cond-list (`if true && &`) makes the flag
///   inert — no part runs, no job. The fully empty **condition**
///   (`if & then`, `if ; then`) is a parse error since task #299 (bash rc 2),
///   so it is no longer a row here;
/// * `if true; & then` is the `; &` leniency (owner **#313**): the slice ends in
///   the plain `&`, so the job is recorded where bash stops with rc 2.
#[test]
fn a_conditional_family_pins_the_measured_divergences() {
    for shape in [
        "if { builtin echo g; } & then builtin echo x; fi",
        "if for i in 1; do builtin echo y; done & then builtin echo x; fi",
        "if false && true & then builtin echo x; else builtin echo f; fi",
        "if true && & then builtin echo x; fi",
    ] {
        let out = run(&format!("{shape}\nbuiltin echo pid=$!; wait"));
        assert_eq!(rc(&out), 0, "shape {shape:?}");
        assert_eq!(
            reported_pid(&out),
            -1,
            "shape {shape:?}: no job is recorded here, stdout={}",
            str::from_utf8(&out.stdout).unwrap_or_default()
        );
    }
    // The compound condition prints in the parent, in order, where bash
    // backgrounds the whole compound (#352): `g` before `x`.
    let out = run("if { builtin echo g; } & then builtin echo x; fi\nwait");
    let stdout = str::from_utf8(&out.stdout).unwrap().to_string();
    assert_eq!(
        (stdout.find('g'), stdout.find('x')),
        (Some(0), Some(2)),
        "stdout={stdout}"
    );
    // The `&` on its own line and the `; &` form: accepted here, and the slice
    // ends in the plain `&`, so the job is recorded (#313; bash stops rc 2).
    for shape in [
        "if true\n&\nthen builtin echo x; fi",
        "if true; & then builtin echo x; fi",
    ] {
        let out = run(&format!("{shape}\nbuiltin echo pid=$!; wait"));
        assert_eq!(rc(&out), 0, "shape {shape:?}: bash rc 2");
        assert!(
            reported_pid(&out) > 0,
            "shape {shape:?}: stdout={}",
            str::from_utf8(&out.stdout).unwrap_or_default()
        );
    }
}
