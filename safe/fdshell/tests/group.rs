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
    let path = std::env::temp_dir().join(format!("group_{tag}_{}.txt", std::process::id()));
    path.to_str().unwrap().to_string()
}

/// Read the redirect target and remove it.
fn body(path: &str) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let _ = std::fs::remove_file(path);
    text
}

/// POSIX #1.2: the group body runs in the current shell, so its assignment is
/// visible after the group. A subshell fork would lose it, so the persisting `x`
/// is the proof the group is not forked.
#[test]
fn group_body_runs_in_the_current_shell() {
    let (out, err, rc) = run("{ x=1; echo $x; }; echo after:$x");
    assert_eq!((out.as_str(), err.as_str(), rc), ("1\nafter:1\n", "", 0));
}

/// POSIX #1.2 + #2.4: the trailing redirect applies to the group and the shell's
/// own fds come back at the group end, so the next command writes to stdout.
#[test]
fn group_redirect_is_scoped_to_the_group() {
    let (a, b) = (temp_path("out"), temp_path("after"));
    let (out, err, rc) = run(&format!("{{ echo a; }} > {a}; echo after > {b}"));
    assert_eq!((out.as_str(), err.as_str(), rc), ("", "", 0));
    assert_eq!(body(&a), "a\n");
    assert_eq!(body(&b), "after\n");
}

/// The group's status is the status of its last body command, and the shell
/// continues with the next statement (`$?` is read after the failing group).
#[test]
fn group_status_is_the_last_body_command() {
    let (out, err, rc) = run("true; { true; }; { false; }; echo rc=$?");
    assert_eq!((out.as_str(), err.as_str(), rc), ("rc=1\n", "", 0));
}

/// A `2>` redirect on the group is scoped, so the process's own stderr stays
/// clean and the target file is what the group wrote to fd 2.
#[test]
fn stderr_redirect_is_scoped() {
    let a = temp_path("err");
    let (out, err, rc) = run(&format!("{{ echo a; }} 2>{a}"));
    assert_eq!((out.as_str(), err.as_str(), rc), ("a\n", "", 0));
    assert_eq!(body(&a), "");
}

/// bash: `{ echo a; } && { echo b; }` — two groups as cond-list parts. The
/// second group runs only when the first succeeds; the group is not a subshell.
#[test]
fn groups_are_cond_list_parts() {
    let (out, err, rc) = run("{ echo a; } && { echo b; }");
    assert_eq!((out.as_str(), err.as_str(), rc), ("a\nb\n", "", 0));
    let (out, err, rc) = run("{ false; } && echo b");
    assert_eq!((out.as_str(), err.as_str(), rc), ("", "", 1));
    let (out, err, rc) = run("{ echo a; } || echo b");
    assert_eq!((out.as_str(), err.as_str(), rc), ("a\n", "", 0));
    let (out, err, rc) = run("false || { echo b; }");
    assert_eq!((out.as_str(), err.as_str(), rc), ("b\n", "", 0));
}

/// The group's `>` redirect is scoped, so a trailing `&&` part after the group
/// writes to the shell's stdout: `b` is printed, `a` is in the file.
#[test]
fn trailing_redirect_then_and_part() {
    let a = temp_path("and");
    let (out, err, rc) = run(&format!("{{ echo a; }}>{a} && echo b"));
    assert_eq!((out.as_str(), err.as_str(), rc), ("b\n", "", 0));
    assert_eq!(body(&a), "a\n");
}

/// Nested groups: the inner group is one body statement, and the outer group's
/// span ends at the statement, past the inner closing brace.
#[test]
fn nested_groups_run_their_bodies() {
    let (out, err, rc) = run("{ { echo a; }; echo b; }");
    assert_eq!((out.as_str(), err.as_str(), rc), ("a\nb\n", "", 0));
}

/// POSIX #1.2: `cd` inside the group persists in the shell, so `pwd` after the
/// group reports the new directory (bash agrees).
#[test]
fn cd_in_the_group_persists() {
    let (out, err, rc) = run("cd /; { cd /tmp; pwd; }; pwd");
    assert_eq!((out.as_str(), err.as_str(), rc), ("/tmp\n/tmp\n", "", 0));
}

/// `exec >file` inside a group is permanent: a group is the current shell, not a
/// subshell, so the redirection is not undone at the group end (bash agrees).
#[test]
fn exec_in_a_group_is_persistent() {
    let a = temp_path("exec");
    let (out, err, rc) = run(&format!("{{ exec >{a} ; }}; echo after"));
    assert_eq!((out.as_str(), err.as_str(), rc), ("", "", 0));
    assert_eq!(body(&a), "after\n");
}

/// `return` in the group body returns from the calling function: the group is a
/// shell construct, not a function boundary.
#[test]
fn return_in_the_group_returns_from_the_function() {
    let (out, err, rc) = run("f() { { return 3; }; }; f; echo rc=$?");
    assert_eq!((out.as_str(), err.as_str(), rc), ("rc=3\n", "", 0));
}

/// `set -e` exits the shell inside the group body, so the next statement never
/// runs and the shell exits with the failing command's status.
#[test]
fn errexit_exits_inside_the_group_body() {
    let (out, err, rc) = run("set -e; { echo a; false; }; echo notreached");
    assert_eq!((out.as_str(), err.as_str(), rc), ("a\n", "", 1));
}

/// `set -x` traces the group's body commands, not the braces: the group line is
/// not a command, and bash prints only the body command traces.
#[test]
fn set_x_traces_the_group_body() {
    let (out, err, rc) = run("set -x; { echo a; }");
    assert_eq!((out.as_str(), rc), ("a\n", 0));
    assert!(err.contains("+ echo a"), "{err}");
    assert!(!err.contains("+ {"), "{err}");
}

/// A here-doc inside the group body is read by the body command (POSIX #2.5).
#[test]
fn here_doc_inside_the_group_body() {
    let (out, err, rc) = run("{ cat <<EOF\nhi\nEOF\n; }");
    assert_eq!((out.as_str(), err.as_str(), rc), ("hi\n", "", 0));
}

/// `eval` parses the group from the evaluated text, so the group is recognized
/// in the eval'd statement, not run as a command named `{`.
#[test]
fn eval_runs_a_group() {
    let (out, err, rc) = run("eval \"{ echo evaled; }\"");
    assert_eq!((out.as_str(), err.as_str(), rc), ("evaled\n", "", 0));
}

/// A group inside `$( … )` is a command-substitution body: its stdout is
/// captured, and the group body keeps its own redirect scope.
#[test]
fn group_inside_command_substitution() {
    let (out, err, rc) = run("echo \"$( { echo a; echo b; } )\"");
    assert_eq!((out.as_str(), err.as_str(), rc), ("a\nb\n", "", 0));
}

/// Brace expansion inside the body is an ordinary body command, so the group
/// braces are not brace expansion (bash prints `a b`).
#[test]
fn brace_expansion_inside_the_body() {
    let (out, err, rc) = run("{ echo {a,b}; }");
    assert_eq!((out.as_str(), err.as_str(), rc), ("a b\n", "", 0));
}

/// `{`/`}` are the block-depth `+1`/`-1`, so a keyword block that holds a group
/// closes at its own keyword: the `if` body and the `while` condition group run.
#[test]
fn group_delimiters_keep_block_depth_balanced() {
    let (out, err, rc) = run("if { true; }; then echo yes; fi");
    assert_eq!((out.as_str(), err.as_str(), rc), ("yes\n", "", 0));
    let (out, err, rc) = run("while { echo w; false; }; do echo body; done");
    assert_eq!((out.as_str(), err.as_str(), rc), ("w\n", "", 0));
}

/// A group in a function body, an `if` body and a `case` arm runs its statements
/// in the shell, so the assignment is visible after the block.
#[test]
fn group_in_block_bodies() {
    let (out, err, rc) = run("f() { x=1; { y=2; }; }; f; echo $x $y");
    assert_eq!((out.as_str(), err.as_str(), rc), ("1 2\n", "", 0));
    let (out, err, rc) = run("case x in x) { x=1; };; esac; echo $x");
    assert_eq!((out.as_str(), err.as_str(), rc), ("1\n", "", 0));
    let (out, err, rc) = run("if true; then { v=5; }; echo $v; fi");
    assert_eq!((out.as_str(), err.as_str(), rc), ("5\n", "", 0));
}

/// `{echo` is a command name, not a group opener (the byte after `{` is not a
/// boundary), so it is command-not-found, as bash's syntax error is there too.
#[test]
fn glued_brace_stays_a_command_word() {
    let (out, err, rc) = run("{echo hi; }");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("\"{echo\" not found"), "{err}");
}

/// A brace in an argument position is a word: `echo {` prints the brace, and the
/// `{` after a pipeline `|` is a command word, not a group.
#[test]
fn brace_in_argument_or_pipeline_position_is_a_word() {
    let (out, err, rc) = run("echo {");
    assert_eq!((out.as_str(), err.as_str(), rc), ("{\n", "", 0));
    let (out, err, rc) = run("echo a| { echo b; }");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("\"{\" not found"), "{err}");
}

/// `{ …; } | cat` is unsupported: a pipeline stage is not a group, and the parse
/// error names the pipe.
#[test]
fn pipeline_stage_group_is_a_parse_error() {
    let (out, err, rc) = run("{ echo a; } | cat");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("unexpected pipe"), "{err}");
}

/// A background group (`{ …; } &`) and a here-doc on a group are unsupported;
/// each pinned parse error names what is missing.
#[test]
fn background_and_here_doc_on_group_are_parse_errors() {
    let (out, err, rc) = run("{ echo a; } &");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(
        err.contains("unexpected character in this context"),
        "{err}"
    );
    let (out, err, rc) = run("{ echo a; } <<EOF");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(
        err.contains("here-doc on a group is not supported"),
        "{err}"
    );
}

/// bash requires the `;` before `}`, an empty group is a syntax error, and a word
/// after the closing brace is not part of the group.
#[test]
fn malformed_groups_are_parse_errors() {
    let (out, err, rc) = run("{ echo a }");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("expected ';' before '}'"), "{err}");
    let (out, err, rc) = run("{ }");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("group is empty"), "{err}");
    let (out, err, rc) = run("{ echo a; } echo b");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("unexpected word after '}'"), "{err}");
}

/// An unclosed group reaches the end of input: the script-level parse error,
/// not a command named `{`.
#[test]
fn unclosed_group_is_a_parse_error() {
    let (out, err, rc) = run("{ echo a");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("parse error"), "{err}");
}
/// A body command that writes to stderr lands its message in the group's `2>`
/// file, and the process's own stderr stays clean.
#[test]
fn body_stderr_goes_to_the_group_file() {
    let f = temp_path("body_err");
    let (out, err, rc) = run(&format!("{{ cat missing; }} 2>{f}"));
    assert_eq!((out.as_str(), err.as_str(), rc), ("", "", 1));
    assert!(
        body(&f).contains("cat: missing"),
        "the message is in the file"
    );
}

/// `break` in the group body breaks the enclosing loop, and the shell continues
/// after the loop (the group is not the loop boundary).
#[test]
fn break_in_the_group_body_breaks_the_loop() {
    let (out, err, rc) = run("while :; do { break; }; done; echo after");
    assert_eq!((out.as_str(), err.as_str(), rc), ("after\n", "", 0));
}

/// A nested group in a function body runs in the shell at call time.
#[test]
fn nested_group_in_a_function_body() {
    let (out, err, rc) = run("f() { { echo nested; }; }; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("nested\n", "", 0));
}

/// `||` after the group: the group's failing status selects the right part.
#[test]
fn or_after_the_group() {
    let (out, err, rc) = run("{ false; } || echo c");
    assert_eq!((out.as_str(), err.as_str(), rc), ("c\n", "", 0));
}

/// The glued `}>out` form writes the group's output to the file.
#[test]
fn glued_redirect_after_the_brace_writes_the_file() {
    let a = temp_path("glued");
    let (out, err, rc) = run(&format!("{{ echo a; }}>{a}"));
    assert_eq!((out.as_str(), err.as_str(), rc), ("", "", 0));
    assert_eq!(body(&a), "a\n");
}

/// A brace-shaped word at top level stays a word: `echo {}` prints the braces
/// (brace expansion leaves `{}` literal), as bash prints `{}`.
#[test]
fn brace_shaped_words_at_top_level_are_data() {
    let (out, err, rc) = run("echo {} ; echo { ; echo }");
    assert_eq!((out.as_str(), err.as_str(), rc), ("{}\n{\n}\n", "", 0));
}

/// A literal `{` argument inside a block body prints the brace, as bash does. A
/// literal `}` argument consumes the block's closing keyword: `if true; then
/// echo }; fi` reports the `'then' body is empty` parse error (rc 1) where bash
/// prints `}` — the accepted cost of counting every brace word as the block
/// depth (the message was `"fi" not found` before the empty-body rule of task
/// #233: the closer consumed by the brace leaves an empty body range).
#[test]
fn literal_brace_argument_in_a_block_body() {
    let (out, err, rc) = run("if true; then echo {; fi");
    assert_eq!((out.as_str(), err.as_str(), rc), ("{\n", "", 0));
    let (out, err, rc) = run("if true; then echo }; fi");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("'then' body is empty"), "{err}");
    let (out, err, rc) = run("case x in x) echo {;; esac");
    assert_eq!((out.as_str(), err.as_str(), rc), ("{\n", "", 0));
}

/// `wait` at the end of a line absorbs the group on the next line as an
/// event-case body, so the group never runs: rc 1 parse error, where bash prints
/// `a` (accepted divergence, pinned).
#[test]
fn wait_line_absorbs_the_next_line_group() {
    let (out, err, rc) = run("wait\n{ echo a; }");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("parse error"), "{err}");
}

/// A brace in an argument position, and a `}` that closes nothing, leave the
/// cond-list group depth at 0, so `&&` / `||` split the line as bash and `master`
/// do — the operator is never swallowed into the command's arguments. Every case
/// is measured against bash.
#[test]
fn argument_position_braces_do_not_swallow_the_cond_operators() {
    let (out, err, rc) = run("echo { && echo b");
    assert_eq!((out.as_str(), err.as_str(), rc), ("{\nb\n", "", 0));
    let (out, err, rc) = run("echo x} && echo y");
    assert_eq!((out.as_str(), err.as_str(), rc), ("x}\ny\n", "", 0));
    let (out, _, rc) = run("echo {a,b} && echo ok");
    assert_eq!((out.as_str(), rc), ("a b\nok\n", 0));
    let (out, _, rc) = run("echo {1..3} && echo ok");
    assert_eq!((out.as_str(), rc), ("1 2 3\nok\n", 0));
    let (out, _, rc) = run("echo x{ && echo y");
    assert_eq!((out.as_str(), rc), ("x{\ny\n", 0));
    let (out, _, rc) = run("echo -n x{ && echo y");
    assert_eq!((out.as_str(), rc), ("x{y\n", 0));
    let (out, _, rc) = run("echo -n x} && echo y");
    assert_eq!((out.as_str(), rc), ("x}y\n", 0));
    // `||` short-circuits on the successful part, so only `{` is printed.
    let (out, err, rc) = run("echo { || echo b");
    assert_eq!((out.as_str(), err.as_str(), rc), ("{\n", "", 0));
    let (out, _, rc) = run("x=1; echo { && echo $x");
    assert_eq!((out.as_str(), rc), ("{\n1\n", 0));
}

/// `&&` / `||` inside a group body is the body's own cond list: the group's span
/// keeps the operator inside, so the split is the operator after the closing
/// brace, and a trailing redirect on a nested group stays with that group.
#[test]
fn cond_operator_inside_the_group_body_is_the_bodys() {
    let (out, err, rc) = run("{ echo a && echo b; }");
    assert_eq!((out.as_str(), err.as_str(), rc), ("a\nb\n", "", 0));
    let (out, _, rc) = run("{ echo a && false; echo c; }");
    assert_eq!((out.as_str(), rc), ("a\nc\n", 0));
    let a = temp_path("nested_and");
    let (out, _, rc) = run(&format!("{{ echo a; }} && {{ echo b; }}>{a}; echo c"));
    assert_eq!((out.as_str(), rc), ("a\nc\n", 0));
    assert_eq!(body(&a), "b\n");
    let (out, _, rc) = run("{ echo a; } && { echo b; } && echo c");
    assert_eq!((out.as_str(), rc), ("a\nb\nc\n", 0));
    let f = temp_path("tail_and");
    let (out, _, rc) = run(&format!("{{ echo a; }}>{f} && echo b"));
    assert_eq!((out.as_str(), rc), ("b\n", 0));
    assert_eq!(body(&f), "a\n");
}

/// The group opener needs a word start, so a quoted brace, a brace inside
/// `$( … )`, a brace glued to a word, and a brace inside a backtick body are all
/// word data and never open a group.
#[test]
fn group_opener_needs_a_word_start() {
    let (out, err, rc) = run("echo \"{ echo a; }\" && echo b");
    assert_eq!(
        (out.as_str(), err.as_str(), rc),
        ("{ echo a; }\nb\n", "", 0)
    );
    // The group inside `$( … )` is the substitution's own body; `&& echo c` is
    // text inside the double quotes, so bash and fdshell print the same line.
    let (out, _, rc) = run("echo \"$( { echo a; echo b; } ) && echo c\"");
    assert_eq!((out.as_str(), rc), ("a\nb && echo c\n", 0));
    // A brace glued to a word is that word's data; the `}` after `;` is a command.
    let (out, err, rc) = run("echo a{ b; }");
    assert_eq!((out.as_str(), rc), ("a{ b\n", 1));
    assert!(err.contains("\"}\" not found"), "{err}");
    // A brace inside a backtick body is data for this shell's scan: the region
    // is one word, the brace never opens a *parent* group and `&& echo c` stays
    // outside it. The body is the child's own text, so the child runs the group
    // (task #216; measured against `bash -c` 5.3.9: `a`, `b`, `c`).
    let (out, _, rc) = run("echo `{ echo a; echo b; }` && echo c");
    assert_eq!((out.as_str(), rc), ("a b\nc\n", 0));
}

/// A function name that is also a block keyword wins the scan: `if()` / `case()`
/// are function definitions, and the bare keyword after `;` is a command (both
/// rc 1, as `master` and bash agree, bash rc 2).
#[test]
fn function_def_beats_the_block_keyword() {
    let (out, err, rc) = run("if() { echo iffunc; }; if");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("parse error"), "{err}");
    let (out, err, rc) = run("case() { echo casefunc; }; case");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("parse error"), "{err}");
}

/// `fdshell` has no bad-substitution diagnostic: `${{a}` expands to an empty
/// word, so the line prints an empty line then `b` rc 0 where bash reports
/// `bad substitution` rc 1 (accepted divergence; `master` agrees).
#[test]
fn bad_substitution_has_no_diagnostic() {
    let (out, err, rc) = run("echo ${{a} && echo b");
    assert_eq!((out.as_str(), err.as_str(), rc), ("\nb\n", "", 0));
}

/// Nesting is capped at 100 groups per construct: 100 nested groups run, 101
/// report `nesting too deep` with rc 1, as the other nested constructs do.
#[test]
fn nesting_cap_applies_to_groups() {
    let open = "{ ";
    let close = "; }";
    let ok = format!("{}echo deep{}", open.repeat(100), close.repeat(100));
    let (out, err, rc) = run(&ok);
    assert_eq!((out.as_str(), err.as_str(), rc), ("deep\n", "", 0));
    let deep = format!("{}echo deep{}", open.repeat(101), close.repeat(101));
    let (out, err, rc) = run(&deep);
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("nesting too deep"), "{err}");
}

/// A group opener after `&&` inside a command substitution must not truncate the
/// statement flushed before it: the substitution's inner text starts with a
/// space, so the run's offset has to skip that whitespace. Pins the offset fix;
/// output matches bash.
#[test]
fn substitution_group_after_operator_keeps_the_run_before_it() {
    let (out, err, rc) = run("echo \"$( echo inner && { echo g; } )\"");
    assert_eq!((out.as_str(), err.as_str(), rc), ("inner\ng\n", "", 0));
}

/// The same truncation after a separator: `inner` survives whole and the group
/// runs, so three lines print (the run cut at the trailing `&` parses as a
/// statement ending in `&` — an error).
#[test]
fn separator_before_the_group_keeps_the_run_offset() {
    let (out, err, rc) = run("echo a;  echo inner && { echo g; }");
    assert_eq!((out.as_str(), err.as_str(), rc), ("a\ninner\ng\n", "", 0));
}

/// A group is a cond part, so the `&&` / `||` short-circuit applies to it: a
/// failing left side skips the group, a failing left side before `||` runs it.
#[test]
fn group_part_respects_the_cond_short_circuit() {
    let (out, err, rc) = run("false && { echo g; }");
    assert_eq!((out.as_str(), err.as_str(), rc), ("", "", 1));
    let (out, err, rc) = run("true && { echo g; }");
    assert_eq!((out.as_str(), err.as_str(), rc), ("g\n", "", 0));
    let (out, err, rc) = run("false || { echo g; } && echo d");
    assert_eq!((out.as_str(), err.as_str(), rc), ("g\nd\n", "", 0));
}

/// The group depth in the cond-boundary scan must stay **positive** across a
/// nested open, because the `max(0)` clamp is applied per closer, not to the
/// running total: an open that `-= 1` runs negative, and the next closer resets
/// it to `0` while the true counter is still positive. That divergence is only
/// observable on the *counted-open, counted-open, closer, then operator*
/// sequence, so the split must land on the operator after the outer `}`. `&&`
/// runs every part; `||` short-circuits after `echo a` succeeds, so only `a`
/// prints. Both are measured against bash 5.3.9 and kill the `+=`→`-=` mutant
/// on `cond/boundary.rs:30` (the mutant splits at the inner `&&`/`||` and
/// reports `group is missing a closing brace` rc 1).
#[test]
fn nested_group_at_a_cond_part_start_keeps_the_body_depth() {
    let (out, err, rc) = run("{ echo a && { echo b; } && echo c; } && echo d");
    assert_eq!((out.as_str(), err.as_str(), rc), ("a\nb\nc\nd\n", "", 0));
    let (out, err, rc) = run("{ echo a || { echo b; } || echo c; } || echo d");
    assert_eq!((out.as_str(), err.as_str(), rc), ("a\n", "", 0));
}

// Task #130: a quoted region is one unit inside a group and inside an `if`
// condition — its `;` and `&&` are region data, so the group's body and the
// condition stay intact.
#[test]
fn quoted_region_inside_groups_and_conditions() {
    let cases: &[(&str, &str)] = &[
        (
            r#"{ echo "$(echo "a; b")"; echo second; }"#,
            "a; b\nsecond\n",
        ),
        (
            r#"if echo "$(echo "a; b")"; then echo yes; fi"#,
            "a; b\nyes\n",
        ),
        (
            r#"if echo "${u:-"a && b"}"; then echo yes; fi"#,
            "a && b\nyes\n",
        ),
        (
            r#"{ echo "$(printf "x && y")"; } && echo after"#,
            "x && y\nafter\n",
        ),
        (r#"echo "$(echo "a; b")" > /dev/null; echo done"#, "done\n"),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(err, "", "{script:?} must report nothing, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}
