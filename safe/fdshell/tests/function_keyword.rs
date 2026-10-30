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

/// Name leniency is inherited from the POSIX form: an option-shaped word is
/// the name (bash rejects it — a documented leniency). A **quoted** name is
/// not a name at all (task #247, POSIX §2.9.1): the definer words are gated
/// on their quote bytes at both granularities, so the shape falls back to the
/// `function` command, exactly as bash's `` `"f"': not a valid identifier ``
/// (fdshell rc 1, bash rc 1).
#[test]
fn definer_name_leniency_is_inherited() {
    let (out, err, rc) = run("function -a f { echo hi; }; -a");
    assert_eq!((out.as_str(), err.as_str(), rc), ("hi\n", "", 0));
    for script in [
        r#"function "f" { echo hi; }; f"#,
        "function 'f'() { echo hi; }; f",
        "function 'f' { echo hi; }; f",
    ] {
        let (out, err, rc) = run(script);
        assert_eq!((out.as_str(), rc), ("", 1), "script={script}: stderr={err}");
        assert!(err.contains("not found"), "script={script}: stderr={err}");
    }
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
    // Task #302: the empty-body spelling of this row (`function f { }; f`) used
    // to define a function whose call ran nothing (rc 0). A definition's body
    // must be a non-empty list, so the shape is a parse error at the definition
    // — the rule's home is `tests/block_bodies.rs`; this row keeps the pin here
    // because the definer *word* rule is what reaches it.
    let (out, err, rc) = run("function f { }; f");
    assert_eq!((out.as_str(), rc), ("", 1), "stderr={err}");
    assert!(
        err.contains("function definition body is empty"),
        "stderr={err}"
    );
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

/// A `)` glued to `function` is the tokenizer's own separator token, so the
/// definer pair is `function` + `)` and no block is claimed: the text runs as
/// commands, the `&&` part is skipped by the failed first part, and nothing is
/// silently dropped (bash rc 2 syntax error).
#[test]
fn glued_close_paren_after_the_keyword_runs_as_commands() {
    let (out, err, rc) = run("function)f g { echo hi; } && echo done");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains(r#""function" not found"#), "stderr={err}");
    assert!(err.contains(r#""}" not found"#), "stderr={err}");
    let (out, err, rc) = run("function)f g { echo hi; }; f");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains(r#""function" not found"#), "stderr={err}");
    assert!(err.contains(r#""f" not found"#), "stderr={err}");
    // The spaced twin behaves identically: the tokens are the same.
    let (out, rc) = {
        let (out, _, rc) = run("function ) f g { echo hi; } && echo done");
        (out, rc)
    };
    assert_eq!((out.as_str(), rc), ("", 1));
    let (out, rc) = {
        let (out, _, rc) = run("function ) f g { echo hi; }; f");
        (out, rc)
    };
    assert_eq!((out.as_str(), rc), ("", 1));
    // A `)` after the name word ends the definer pair, as in the POSIX form.
    let (out, err, rc) = run("function f)g { echo hi; }; f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("hi\n", "", 0));
}

/// The mid-run definition gap (task #168) closed with task #232: a
/// `function name { … }` written after `&&` is a cond part — the segment
/// scanner skips its span inside the run, so the definition parses, the
/// gated call runs it, and bash's behavior is matched exactly.
#[test]
fn mid_run_definition_is_a_cond_part() {
    let (out, err, rc) = run("true && function f { echo hi; }; f");
    assert_eq!((out.as_str(), rc), ("hi\n", 0), "stderr={err}");
    let (out, err, rc) = run("false && function g { echo hi; }; g");
    assert_eq!((out.as_str(), rc), ("", 1), "stderr={err}");
}

/// `set -x` traces the call's body, not the definition (stable, same as the POSIX form).
#[test]
fn xtrace_traces_the_call_not_the_definition() {
    let (out, err, rc) = run("set -x; function f { echo hi; }; f");
    assert_eq!((out.as_str(), rc), ("hi\n", 0));
    assert!(err.contains("+ echo hi"), "stderr={err}");
    assert!(!err.contains("function f"), "stderr={err}");
}

/// A `& ;` inside a function body is rejected at the **call** (task #355):
/// the body text carries the glued `;`, the call's scan marks the split
/// invalid and runs nothing — bash rc 2, fdshell rc 1, no output. A
/// definition that is never called parses lazily: rc 0 (bash parses the body
/// at definition time, rc 2 — the documented lazy-parse divergence of
/// `tests/block_bodies.rs::ampersand_only_body_is_not_an_empty_body`).
#[test]
fn ampersand_then_semicolon_in_a_function_body_is_rejected_at_the_call() {
    let (out, err, rc) = run("f(){ echo a & ; }; f");
    assert_eq!((out.as_str(), rc), ("", 1), "bash rc 2: {err}");
    assert!(err.contains("unexpected character"), "{err}");
    let (out, err, rc) = run("function f { echo a & ; }; f");
    assert_eq!((out.as_str(), rc), ("", 1), "bash rc 2: {err}");
    assert!(err.contains("unexpected character"), "{err}");
    let (out, _err, rc) = run("f(){ echo a & ; }");
    assert_eq!(
        (out.as_str(), rc),
        ("", 0),
        "bash rc 2: lazy-parse divergence"
    );
}

/// The function-body granularity of task #492 (POSIX #2.9): a function body is
/// re-scanned, so a line-ending `&&`/`||` joins the list onto the next physical
/// line. Measured 2026-10-30 against bash 5.3.9 and this build.
#[test]
fn a_line_ending_on_a_cond_operator_continues_a_function_body() {
    for (script, want, rc) in [
        ("f(){ echo a ||\necho b; }; f", "a\n", 0),
        ("f(){ false &&\necho b; }; f", "", 1),
        ("f(){ true ||\necho b; }; f", "", 0),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, rc), "{script}: {err}");
    }
}

/// A definer word that carries a `$( … )` region is never a name: the production
/// matches the statement, the word lands in the name slot, and bash rejects the
/// name at parse time (`` `$(echo x)': not a valid identifier ``). fdshell names
/// the word its tokenizer sees — with the glued `(` bash does not read into the
/// name — and stops at its shell-wide parse-error rc 1 with nothing run (the
/// rc-1-vs-bash-2 family, README "Words and quoting"; task #368).
#[test]
fn a_definer_word_carrying_a_region_reports_the_name() {
    for (script, word) in [
        ("f$( echo x )() { :; }", "f$( echo x )("),
        ("$( echo f )() { :; }", "$( echo f )("),
        ("function $( echo x ) { :; }", "$( echo x )"),
    ] {
        let (out, err, rc) = run(script);
        assert_eq!((out.as_str(), rc), ("", 1), "script={script}: stderr={err}");
        let want = format!("'{word}' is not a valid function name");
        assert!(err.contains(&want), "script={script}: stderr={err}");
        assert!(err.contains("Suggestion"), "script={script}: stderr={err}");
    }
    // The continuation divergence is frozen here: bash keeps going after its own
    // report (`; echo tail` prints `tail` at rc 0), while fdshell's parse error
    // aborts the script, so the tail never runs.
    let (out, err, rc) = run("$( echo f )() { :; }; echo tail");
    assert_eq!((out.as_str(), rc), ("", 1), "stderr={err}");
    assert!(err.contains("is not a valid function name"), "stderr={err}");
}

/// A definition must be followed by `;`, a newline or the end of input for bash,
/// never by a word: `f() { echo hi; } $( echo x )` is a syntax error there
/// (rc 2). fdshell accepts the define-and-call idiom, so the trailing word
/// **runs** — pinned on both sides of the boundary: the `;`-twin control runs in
/// both shells at rc 0, the define-and-call spelling prints `hi` at rc 0 (its
/// owner is `definer_extra_words_and_trailing_run_are_dropped`), and this row is
/// pinned as measured (task #368, no behavior change).
#[test]
fn a_definition_behind_a_trailing_word_runs_it() {
    let (out, err, rc) = run("f() { echo hi; } $( echo x )");
    assert_eq!((out.as_str(), rc), ("", 1), "bash rc 2: stderr={err}");
    assert!(err.contains(r#""x" not found"#), "stderr={err}");
    let (out, err, rc) = run("f() { :; }; echo ok");
    assert_eq!((out.as_str(), err.as_str(), rc), ("ok\n", "", 0));
    let (out, err, rc) = run("function f { echo hi; } f");
    assert_eq!((out.as_str(), err.as_str(), rc), ("hi\n", "", 0));
}
