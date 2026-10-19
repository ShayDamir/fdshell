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

fn contains_all(out: &std::process::Output, want: &[&str]) {
    let stdout = str::from_utf8(&out.stdout).unwrap().to_string();
    for w in want {
        assert!(stdout.contains(w), "want {w} in stdout={stdout}");
    }
}

/// `echo hi &` is a background job: it runs, the shell continues, `wait` reaps
/// it, and the script's status is 0.
#[test]
fn async_statement_runs_and_is_reaped_by_wait() {
    let out = run("builtin echo hi &\nwait");
    assert_eq!(rc(&out), 0);
    assert_eq!(lines(&out), ["hi"]);
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
    let pid = str::from_utf8(&out.stdout)
        .unwrap()
        .split("pid=")
        .nth(1)
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap();
    assert!(pid > 0, "stdout={:?}", str::from_utf8(&out.stdout));
}

/// A pipeline with a trailing `&` is one job, reaped by one `wait`.
#[test]
fn pipeline_is_one_async_job() {
    let out = run("builtin echo a | builtin echo b &\nwait");
    assert_eq!(rc(&out), 0);
    contains_all(&out, &["b"]);
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
/// family row in `POSIX_COMPATIBILITY.md` (a parse error) is retired.
#[test]
fn here_string_on_fd3_with_trailing_ampersand_matches_bash() {
    let out = run("cat 3<<<x &\nwait");
    assert_eq!(rc(&out), 0);
    assert!(
        lines(&out).is_empty(),
        "stdout={:?}",
        str::from_utf8(&out.stdout)
    );
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
    // (an async `&` here would have launched `echo` in the background).
    let out = run("builtin echo three 1>&3");
    assert_eq!(rc(&out), 1);
    assert!(
        str::from_utf8(&out.stderr)
            .unwrap()
            .contains("fd 3 is not open"),
        "stderr={:?}",
        str::from_utf8(&out.stderr)
    );
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
