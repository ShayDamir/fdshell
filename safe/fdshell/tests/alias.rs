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

/// A pipeline whose downstream stage never reads stdin (e.g. `echo | echo`)
/// can produce a benign "Broken pipe" on the upstream: the downstream exits
/// before the upstream writes, and nothing else holds the pipe's read end.
/// The real failure these tests care about is an unexpanded alias ("not
/// found"), so assert on that instead of requiring empty stderr.
fn assert_no_alias_error(err: &str) {
    assert!(!err.contains("not found"), "stderr={err:?}");
}

#[test]
fn alias_expands_command_word() {
    let (out, err, code) = run(r#"alias ll="echo aliased"; ll"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "aliased\n");
}

#[test]
fn alias_replaces_with_command_and_keeps_args() {
    let (out, err, code) = run(r#"alias x=echo; x hello world"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hello world\n");
}

#[test]
fn aliases_chain() {
    let (out, err, code) = run(r#"alias a=b; alias b="echo deep"; a"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "deep\n");
}

#[test]
fn alias_lists_definitions() {
    let (out, err, code) = run(r#"alias zz="echo z"; alias aa="echo a"; alias"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.contains("alias aa='echo a'"), "stdout={out:?}");
    assert!(out.contains("alias zz='echo z'"), "stdout={out:?}");
}

#[test]
fn alias_displays_single_definition() {
    let (out, err, code) = run(r#"alias ll="echo aliased"; alias ll"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "alias ll='echo aliased'\n");
}

#[test]
fn unalias_removes_expansion() {
    let (out, err, code) = run(r#"alias x="echo via"; x; unalias x; x"#);
    assert_ne!(code, 0);
    assert!(err.contains("not found"), "stderr={err:?}");
    assert_eq!(out, "via\n");
}

#[test]
fn expand_aliases_gate_disables_expansion() {
    let (_out, err, code) = run(r#"alias x="echo via"; shopt -u expand_aliases; x"#);
    assert_ne!(code, 0);
    assert!(err.contains("not found"), "stderr={err:?}");
}

#[test]
fn alias_after_leading_whitespace() {
    let (out, err, code) = run("alias ll=\"echo aliased\";    ll");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "aliased\n");
}

#[test]
fn reserved_words_cannot_be_aliased() {
    let (out, err, code) = run(r#"alias if="echo nope"; if true; then echo ok; fi"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "ok\n");
}

#[test]
fn alias_of_unknown_name_errors() {
    let (_out, err, code) = run("alias nosuch");
    assert_ne!(code, 0);
    assert!(err.contains("nosuch"), "stderr={err:?}");
    let (_out, err, code) = run("unalias nosuch");
    assert_ne!(code, 0);
    assert!(err.contains("nosuch"), "stderr={err:?}");
}

#[test]
fn alias_value_with_quote_round_trips() {
    let (out, err, code) = run(r#"alias q="echo a'b"; alias q"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "alias q='echo a'\\''b'\n");
}

#[test]
fn alias_expands_after_pipe() {
    let (out, err, code) = run(r#"alias g="echo got"; echo hi | g"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "got\n");
}

#[test]
fn alias_expands_after_unspaced_pipe() {
    // `echo hi|g` rewrites to `echo hi|cat`: the pre-pipe word and the
    // pipe survive, the aliased word is replaced in place. Pre-fix the stale
    // post-pipe span made the rewrite eat `hi|` (line became `echo echo cat`,
    // printing `echo cat`). The downstream `cat` reads the pipe, so the test
    // cannot trip the `echo | echo` broken-pipe race.
    let (out, err, code) = run(r#"alias g=cat; echo hi|g"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.is_empty(), "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn alias_expands_in_every_pipeline_segment() {
    // The first segment's stdout flows into the pipe; an unexpanded `g` there
    // would surface as "not found" on stderr.
    let (out, err, code) = run(r#"alias g="echo got"; g x | g y"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_no_alias_error(&err);
    assert_eq!(out, "got y\n");
}

#[test]
fn alias_expands_after_pipe_and_cond_operators() {
    let (out, err, code) = run(r#"alias g="echo got"; g x && g y | g z"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_no_alias_error(&err);
    assert_eq!(out, "got x\ngot z\n");
}

#[test]
fn alias_chains_after_pipe() {
    let (out, err, code) = run(r#"alias a=b; alias b="echo deep"; a | b"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_no_alias_error(&err);
    assert_eq!(out, "deep\n");
}

#[test]
fn alias_ignores_pipe_inside_quotes() {
    let (out, err, code) = run(r#"alias g="echo got"; echo "a|b" | g"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "got\n");
}

/// Rule (task #471): alias expansion is **not** gated to the interactive
/// shell — fdshell expands in every input mode (REPL, `-c`, script files), so
/// an aliased name runs its alias where bash runs the command. bash substitutes
/// aliases in the interactive shell only, so each row here is an accepted
/// divergence; every assertion is fdshell's measured value, with bash 5.3.9's
/// measured answer named in the message. `expand_aliases` (on by default) is
/// the user's control — `expand_aliases_gate_disables_expansion` pins the
/// opt-out.
#[test]
fn expansion_is_not_gated_to_interactive_accepted_divergences() {
    // (script, fdshell stdout, fdshell stderr needle — empty means "no stderr",
    //  fdshell rc, bash's measured answer)
    for (script, out_want, err_needle, code, bash_want) in [
        (
            r#"alias e=echo; e hi"#,
            "hi\n",
            "",
            0,
            "rc 127 `e: command not found`",
        ),
        (
            r#"alias x=true; x; echo rc=$?"#,
            "rc=0\n",
            "",
            0,
            "`x: command not found` then `rc=127`",
        ),
        (
            r#"alias ls=x; ls"#,
            "",
            r#"resolve command path: "x""#,
            1,
            "runs the real ls (directory listing), rc 0",
        ),
        (
            r#"alias e=echo; case abc in (a*) e m;; esac"#,
            "m\n",
            "",
            0,
            "rc 127 `e: command not found`",
        ),
    ] {
        let (out, err, got) = run(script);
        assert_eq!(
            got, code,
            "{script}: rc (bash: {bash_want}), stderr={err:?}"
        );
        assert_eq!(out, out_want, "{script}: stdout (bash: {bash_want})");
        if err_needle.is_empty() {
            assert!(
                err.is_empty(),
                "{script}: stderr={err:?} (bash: {bash_want})"
            );
        } else {
            // The wording is the resolve layer's and debug builds add source
            // frames, so pin substrings, never exact bytes.
            assert!(
                err.contains(err_needle),
                "{script}: stderr={err:?} (bash: {bash_want})"
            );
        }
    }
}

/// The mirror of `expand_aliases_gate_disables_expansion`: a mid-script
/// `shopt -s` acts on the statements behind it, because fdshell runs each
/// statement as it reads it. bash parses the whole `-c` input before running
/// anything, so both rows stay divergences (bash 5.3.9 rc 127,
/// `e: command not found`); the first row is the task's measured one, the
/// second is the toggle's own proof.
#[test]
fn a_mid_script_shopt_enable_takes_effect_on_later_statements() {
    let (out, err, code) = run("shopt -s expand_aliases; alias e=echo; e hi");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err:?}");
    let (out, err, code) =
        run("shopt -u expand_aliases; alias e=echo; shopt -s expand_aliases; e hi");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err:?}");
}
