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

/// The bash/ksh `function name { … }` keyword form runs like the POSIX `name() { … }`
/// form: the body is the function, and the call dispatches to it.
#[test]
fn keyword_form_runs_its_body() {
    let (out, err, rc) = run("function f { echo \"in f\"; }; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("in f\n", "", 0));
}

/// All four definer shapes (glued parens, spaced parens, split parens, no parens)
/// take the same name, as bash.
#[test]
fn definer_shapes_agree() {
    for (script, want) in [
        ("function f { echo paren; }; f", "paren\n"),
        ("function f() { echo paren; }; f", "paren\n"),
        ("function f () { echo paren; }; f", "paren\n"),
        ("function f ( ) { echo paren; }; f", "paren\n"),
    ] {
        let (out, err, rc) = run(script);
        assert_eq!(
            (out.as_str(), err.as_str(), rc),
            (want, "", 0),
            "script={script}"
        );
    }
}

/// The call swaps positional parameters into the body, first-touch.
#[test]
fn call_arguments_reach_the_body() {
    let (out, err, rc) = run("function f { echo \"$1\"; }; f xy");
    assert_eq!((out.as_str(), err.as_str(), rc), ("xy\n", "", 0));
}

/// The keyword form pushes the same call frame, so `local` is scoped to the call.
#[test]
fn local_is_scoped_to_the_keyword_call() {
    let (out, err, rc) = run("function f { local x=1; }; f; echo \"${x:-unset}\"");
    assert_eq!((out.as_str(), err.as_str(), rc), ("unset\n", "", 0));
}

/// Nested keyword blocks: `function` inside a `function` body, and a group, an
/// `if`, and brace expansion in the body.
#[test]
fn keyword_bodies_nest() {
    let (out, err, rc) = run("function f { function g { echo inner; }; g; }; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("inner\n", "", 0));
    let (out, err, rc) = run("function f { if true; then echo hi; fi; }; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("hi\n", "", 0));
    let (out, err, rc) = run("function f { { echo nested; }; }; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("nested\n", "", 0));
    let (out, err, rc) = run("function f { echo {a,b}; }; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("a b\n", "", 0));
}

/// A heredoc body inside the block is opaque: its `}` lines do not close the block.
#[test]
fn heredoc_body_is_opaque() {
    let (out, err, rc) = run("function f { cat <<EOF\nhi\nEOF\n}; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("hi\n", "", 0));
}

/// A definition at the start of a cond-list run keeps the run's own parts: the
/// `&&` part runs, and the call runs.
#[test]
fn definition_at_run_start_keeps_the_cond_parts() {
    let (out, err, rc) = run("function f { echo hi; } && echo done; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("done\nhi\n", "", 0));
}

/// `function` is a reserved word, so `alias function=echo` never replaces the
/// keyword position (as bash).
#[test]
fn function_keyword_is_not_aliased() {
    let (out, err, rc) = run("alias function=echo; function f { echo hi; }; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("hi\n", "", 0));
}

/// A keyword-form definition is stored in the function table, so `type` reports it.
#[test]
fn type_reports_the_keyword_definition() {
    let (out, err, rc) = run("function f { echo hi; }; type f");
    assert_eq!(
        (out.as_str(), err.as_str(), rc),
        ("f is a shell function\n", "", 0)
    );
}

/// `function f` with no brace on the line claims an open block, and the missing
/// closing brace is a parse error (bash rc 2 syntax error, fdshell rc 1).
#[test]
fn keyword_form_without_a_brace_is_a_parse_error() {
    let (out, err, rc) = run("function f");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("parse error"), "stderr={err}");
    let (out, rc) = {
        let (out, _, rc) = run("function f; echo after");
        (out, rc)
    };
    assert_eq!((out.as_str(), rc), ("", 1));
}

/// A definer name with arguments is not a definition; the shapes fall back to the
/// `function` command, which is not found (bash rc 2 syntax error).
#[test]
fn keyword_form_without_a_definer_name_is_a_command() {
    let (out, err, rc) = run("function f(x) { echo hi; }; f");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(
        err.contains(r#"failed to resolve command path: "function""#),
        "stderr={err}"
    );
    assert!(err.contains(r#""function" not found"#), "stderr={err}");
    assert!(err.contains(r#""}" not found"#), "stderr={err}");
    assert!(err.contains(r#""f" not found"#), "stderr={err}");
    let (out, err, rc) = run("function { echo hi; }");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains(r#""function" not found"#), "stderr={err}");
    assert!(err.contains(r#""}" not found"#), "stderr={err}");
    let (out, err, rc) = run("function");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains(r#""function" not found"#), "stderr={err}");
}

/// Name leniency is inherited from the POSIX form: an option-shaped word is the
/// name, and a quoted name is stored unquoted (bash rejects both).
#[test]
fn definer_name_leniency_is_inherited() {
    let (out, err, rc) = run("function -a f { echo hi; }; -a");
    assert_eq!((out.as_str(), err.as_str(), rc), ("hi\n", "", 0));
    let (out, err, rc) = run("function \"f\" { echo hi; }; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("hi\n", "", 0));
    // A name that is also a block keyword: the definition wins, and the trailing
    // bare keyword is a parse error (as the POSIX `if() { … }; if` pin).
    let (out, err, rc) = run("function if { echo hi; }; if");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("parse error"), "stderr={err}");
}

/// The word `function` itself is a POSIX-form name (`function() { … }`), as master
/// accepts it (bash rc 2 syntax error).
#[test]
fn keyword_word_is_a_posix_form_name() {
    let (out, err, rc) = run("function() { echo fn; }; function");
    assert_eq!((out.as_str(), err.as_str(), rc), ("fn\n", "", 0));
}

/// The definer rule stops at the name word, so the POSIX trailing-word leniency
/// carries over, and a trailing run after `}` is dropped (bash rc 2 for both).
#[test]
fn definer_extra_words_and_trailing_run_are_dropped() {
    let (out, err, rc) = run("function f x { echo hi; }; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("hi\n", "", 0));
    let (out, err, rc) = run("function f { echo hi; } f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("hi\n", "", 0));
    let (out, err, rc) = run("function f { }; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("", "", 0));
}

/// Pathological `}{`: the byte layer claims the span, the token layer sees no `{`
/// token, so the text runs as commands (`function` not found, bash rc 2).
#[test]
fn glued_brace_after_the_name_is_not_a_block() {
    let (out, err, rc) = run("function f }{ echo hi; }");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains(r#""function" not found"#), "stderr={err}");
}

/// The definer rule reads the run's first word, so a `function` word in an
/// argument position is data.
#[test]
fn function_words_in_argument_position_are_data() {
    let (out, err, rc) = run("echo function f { x }");
    assert_eq!(
        (out.as_str(), err.as_str(), rc),
        ("function f { x }\n", "", 0)
    );
}

/// A mid-run definition is the inherited cond-list gap (task #168): the cond part
/// has no closing brace, so it is a parse error rc 1 where bash runs it.
#[test]
fn mid_run_definition_is_a_parse_error() {
    let (out, err, rc) = run("true && function f { echo hi; }; f");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(
        err.contains("function definition is missing a closing brace"),
        "stderr={err}"
    );
}

/// `set -x` traces the call's body, not the definition (stable, same as the POSIX form).
#[test]
fn xtrace_traces_the_call_not_the_definition() {
    let (out, err, rc) = run("set -x; function f { echo hi; }; f");
    assert_eq!((out.as_str(), rc), ("hi\n", 0));
    assert!(err.contains("+ echo hi"), "stderr={err}");
    assert!(!err.contains("function f"), "stderr={err}");
}
