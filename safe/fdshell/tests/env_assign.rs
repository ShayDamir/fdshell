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

#[test]
fn scoped_prefix_is_not_persisted() {
    let (out, err, code) = run("FOO=bar true; builtin echo ${FOO:-unset}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "unset\n");
}

#[test]
fn scoped_prefix_is_exported_to_external_child() {
    // Unquoted grep pattern: quoted args in pipeline stages are a separate
    // pre-existing issue, not part of this task.
    let (out, err, code) = run("FOO=bar env | grep ^FOO=");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "FOO=bar\n");
}

#[test]
fn bare_compound_assignment_persists() {
    let (out, err, code) = run("FOO=bar BAZ=qux; builtin echo $FOO$BAZ");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "barqux\n");
}

#[test]
fn first_touch_value_is_restored() {
    // Bash restores the value from before the prefix: `FOO=old FOO=new`
    // records the original once, so the shell's `FOO=pre` comes back.
    let (out, err, code) = run("FOO=pre; FOO=old FOO=new cd /tmp; builtin echo $FOO");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "pre\n");
}

#[test]
fn first_touch_of_unset_var_restores_unset() {
    let (out, err, code) = run("FOO=old FOO=new cd /tmp; builtin echo ${FOO:-unset}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "unset\n");
}

#[test]
fn scoped_prefix_does_not_clear_last_arg() {
    let (out, err, code) = run("FOO=bar true hi; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn prefix_value_is_expanded_against_shell_state() {
    let (out, err, code) = run("A=1; FOO=$A true; builtin echo ${FOO:-unset}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "unset\n");
    let (out, err, code) = run("A=1; FOO=$A env | grep ^FOO=");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "FOO=1\n");
}

#[test]
fn scoped_ifs_does_not_split_the_commands_own_words() {
    let (out, err, code) = run("IFS=: builtin echo a:b");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a:b\n");
}

#[test]
fn scoped_ifs_leaves_a_literal_word_whole() {
    // Task #133 pin: the scoped value is installed when the command runs, and
    // the word performed no expansion, so its `:` is no split point — one
    // argument reaches the builtin (`IFS=:; builtin printf "[%s]" a:b` in bash
    // prints `[a:b]`).
    let (out, err, code) = run(r#"IFS=: builtin printf "[%s]" a:b"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a:b]");
}

#[test]
fn scoped_ifs_is_restored_afterward() {
    let (out, err, code) = run("IFS=: true; x=a:b; builtin echo $x");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a:b\n");
}

#[test]
fn scoped_ifs_is_visible_to_the_command() {
    // The external child sees the scoped IFS in its environment.
    let (out, err, code) = run("IFS=: env | grep ^IFS=");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "IFS=:\n");
}

#[test]
fn scoped_prefix_is_visible_inside_a_function() {
    let (out, err, code) =
        run("f() { builtin echo in=$FOO; }; FOO=bar f; builtin echo out=${FOO:-unset}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "in=bar\nout=unset\n");
}

#[test]
fn bare_assignment_separated_by_semicolon_persists() {
    let (out, err, code) = run("FOO=bar; builtin echo $FOO");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "bar\n");
}

#[test]
fn scoped_prefix_before_pipeline_is_a_parse_error() {
    let (_out, err, code) = run("FOO=bar | cat");
    assert_ne!(code, 0);
    assert!(!err.is_empty(), "expected a parse error on stderr");
}

#[test]
fn scoped_prefix_before_keyword_runs_keyword_as_command() {
    // Divergence from bash: `if` becomes a plain (not found) command.
    let (_out, err, code) = run("FOO=bar if true; then :; fi");
    assert_ne!(code, 0);
    assert!(
        err.contains(r#"failed to resolve command path: "if""#),
        "stderr={err:?}"
    );
}

#[test]
fn word_with_equals_after_command_is_an_arg() {
    let (out, err, code) = run("true FOO=bar; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "FOO=bar\n");
}

#[test]
fn scoped_prefix_on_intercepted_command_is_visible() {
    let (out, err, code) = run("FOO=bar eval \"builtin echo $FOO\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "bar\n");
    let (out, err, code) = run("FOO=bar eval \"builtin echo $FOO\"; builtin echo ${FOO:-unset}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "bar\nunset\n");
}

/// Task #217, the headline case: a `\<char>` pair written **inside double
/// quotes** keeps its backslash (POSIX #4.2). With the empty quote mask the
/// value folded as an unquoted escape pair and printed `[a b]`.
#[test]
fn quoted_escape_pair_in_a_value_keeps_its_backslash() {
    let (out, err, code) = run(r#"x="a\ b"; printf "[%s]\n" "$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a\\ b]\n");
}

/// The escape pairs POSIX #4.1 and #4.2 agree on are unchanged by the mask:
/// `\\` collapses to one backslash, `\$` is a literal `$` (never expanded).
#[test]
fn backslash_and_dollar_pairs_are_unchanged_by_the_mask() {
    let (out, err, code) = run(r#"x="a\\b"; printf "[%s]\n" "$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a\\b]\n");
    let (out, err, code) = run(r#"b=1; x="a\$b"; printf "[%s]\n" "$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a$b]\n");
}

/// A single-quoted value is stored in the double-quote escape form, whose
/// `\$`/`\\` pairs fold identically under both mask arms — the same output
/// with and without the mask (LESSONS: "a single-quoted literal is stored in
/// the double-quote escape form").
#[test]
fn single_quoted_value_is_mask_independent() {
    let (out, err, code) = run(r#"x='a\ b'; printf "[%s]\n" "$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a\\ b]\n");
}

/// The first byte of the value is an escape pair: the mask slice must start at
/// the value's first byte, not at the `=` (the `name.len() + 1` offset).
#[test]
fn escape_pair_at_the_start_of_the_value_keeps_its_dollar() {
    let (out, err, code) = run(r#"b=1; x="\$b"; printf "[%s]\n" "$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[$b]\n");
}

/// A value is stored **whole**: `$( … )` with a space and a pattern character
/// reach the variable verbatim — no IFS splitting, no pathname expansion
/// (the mask changes folding only).
#[test]
fn value_is_stored_whole_without_splitting_or_globbing() {
    let (out, err, code) = run(r#"x="$(printf "a b")"; printf "[%s]\n" "$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a b]\n");
    let (out, err, code) = run(r#"x="a *"; printf "[%s]\n" "$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a *]\n");
}

/// The store keeps the whole value; the *use* of it still splits on IFS, and
/// the split fields carry the folded bytes — bash splits `a\ b` into `a\` and
/// `b`, fdshell (with the empty mask) produced `a` and `b`.
#[test]
fn use_of_the_value_splits_and_keeps_the_quoted_backslash() {
    let (out, err, code) = run(r#"x="a\ b"; printf "[%s]\n" $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a\\]\n[b]\n");
}

/// The scoped-prefix form reaches the child verbatim.
#[test]
fn scoped_prefix_value_reaches_the_child_verbatim() {
    let (out, err, code) = run(r#"x="a\ b" env | grep '^x='"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "x=a\\ b\n");
}

/// The offset pin in integration form (LESSONS: pin the offset with words of
/// **different** quoting): the quoted pair keeps its backslash, the unquoted
/// pair folds, in the same statement.
#[test]
fn two_prefixed_values_with_different_quoting_keep_their_own_masks() {
    let (out, err, code) = run(r#"x="a\ b" y=b\ c env | grep -E '^x=|^y=' | sort"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "x=a\\ b\ny=b c\n");
}

/// The same folding visible in `IFS` itself: quoted `\t` is a backslash and a
/// `t` (bash's three-byte `IFS`), not the tab an unquoted escape pair folds to
/// — so the split points are the space and the backslash, and `t` is not one.
#[test]
fn quoted_ifs_keeps_the_backslash_as_a_split_byte() {
    let (out, err, code) = run(r#"IFS=" \t"; printf "[%s]" "$IFS""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[ \\t]", "bash: [ \\t]");
    let (out, err, code) = run(r#"IFS=" \t"; v=a\ b; printf "[%s]" $v"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a][b]", "bash: [a][b]");
}

/// Task #223: the **assignment path** of a quoted expansion glued to literal
/// text — the value of an assignment keeps every byte, and the glued literal
/// never extends the parameter name (`vb` is bound and must not win). bash:
/// `v=q; w="$v"b,c; printf "[%s]" "$w"` → `[qb,c]`. The same shapes run
/// through the **split path** in
/// `tests/wordsplit.rs::quoted_expansion_glued_to_literal_text_keeps_every_byte`;
/// this pin measures the folding of the value into the environment.
#[test]
fn quoted_expansion_glued_to_literal_text_in_an_assignment_value() {
    let cases: &[(&str, &str)] = &[
        (r#"IFS=,; v=q; w="$v"b,c; printf "[%s]" "$w""#, "[qb,c]"),
        (r#"IFS=,; v=q; w=x"$v"b,c; printf "[%s]" "$w""#, "[xqb,c]"),
        (r#"IFS=,; v=""; w="$v"a,b; printf "[%s]" "$w""#, "[a,b]"),
        (r#"v=q; vb=VB; w="$v"bc; printf "[%s]" "$w""#, "[qbc]"),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(out, *want, "{script:?}");
    }
}

// Accepted divergence (task #222, to be closed by task #288): a **quoted**
// `$@` in a value has no join byte. Task #222's splitter needs the
// separator-free positional text (`IFS=,; set -- "a b" c; printf %s x"$@"y`
// → `[xa b][cy]`, pinned in `tests/wordsplit.rs`), and an assignment never
// splits — it stores the substituted text whole — so the positionals come out
// glued where bash joins them with a space. bash's non-word `$@` join is
// always a single space, whatever `IFS` is; the values below are the measured
// fdshell text, and the bash value is quoted beside each case.
#[test]
fn quoted_dollar_at_in_an_assignment_value_is_glued() {
    let cases: &[(&str, &str)] = &[
        // bash: `[a b c]` then `[xa b c]`.
        (
            r#"set -- "a b" c; v="$@"; printf "[%s]\n" "$v" "x$v""#,
            "[a bc]\n[xa bc]\n",
        ),
        // `IFS=,`: bash still joins with a space (`[a b c]`).
        (
            r#"IFS=,; set -- "a b" c; v="$@"; printf "[%s]" "$v""#,
            "[a bc]",
        ),
        // The scoped form reaches the child the same way (bash: `v=a b c`).
        (r#"set -- "a b" c; v="$@" env | grep "^v=""#, "v=a bc\n"),
        // Guard: the unquoted spelling joins with the first `IFS` byte
        // (master's rule) and with the default `IFS` that byte is a space,
        // so it matches bash (`[a b c]`, `[xa b c]`).
        (
            r#"set -- "a b" c; v=$@; printf "[%s]\n" "$v" "x$v""#,
            "[a b c]\n[xa b c]\n",
        ),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}
