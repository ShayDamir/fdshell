#![allow(clippy::unwrap_used)]

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

// --- POSIX 2.6.2: an unset parameter expands to the empty string ---

#[test]
fn unset_braced_in_quotes_is_empty() {
    let (out, err, code) = run(r#"echo "[$undefined]""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[]\n");
}

#[test]
fn unset_bare_names_concatenate_to_empty() {
    let (out, err, code) = run("echo [$x$y]");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[]\n");
}

#[test]
fn two_unset_braced_names_are_empty() {
    // The word collapses to nothing, so `echo` prints the blank line.
    let (out, err, code) = run(r#"echo "${a}${b}""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "\n");
}

#[test]
fn unset_inside_a_word_keeps_the_neighbours() {
    let (out, _err, code) = run(r#"echo "[${x}y]""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[y]\n");
    let (out, _err, code) = run(r#"echo "[a${x}b${y}c]""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[abc]\n");
}

#[test]
fn unset_adjacent_to_literal_text() {
    let (out, _err, code) = run("echo ${unset}x");
    assert_eq!(code, 0);
    assert_eq!(out, "x\n");
    let (out, _err, code) = run("echo x${unset}y");
    assert_eq!(code, 0);
    assert_eq!(out, "xy\n");
}

#[test]
fn unset_expanded_as_a_printf_argument() {
    let (out, _err, code) = run(r#"printf "[%s]\n" "$unset""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[]\n");
}

// --- POSIX 2.7.1: `${#name}` of an unset parameter is length 0 ---

#[test]
fn unset_length_is_zero() {
    let (out, err, code) = run(r#"echo "${#undefined}""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "0\n");
    let (out, _err, code) = run(r#"echo "${#undefined}x""#);
    assert_eq!(code, 0);
    assert_eq!(out, "0x\n");
}

#[test]
fn empty_length_name_is_zero_under_nounset() {
    // `${#}` is the length of the empty name. Bash prints `0` at rc 0 with and
    // without `set -u`, so the empty name must not reach the nounset bail.
    let (out, err, code) = run(r#"printf "%s\n" "[${#}]""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[0]\n");
    let (out, err, code) = run(r#"set -u; printf "%s\n" "[${#}]""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[0]\n");
    let (out, err, code) = run(r#"set -u; echo "[${#}y]""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[0y]\n");
}

// --- `${!name}`: indirect expansion ---

#[test]
fn indirect_with_unset_target_is_empty() {
    let (out, err, code) = run(r#"q=missing; echo "[${!q}]""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[]\n");
}

#[test]
fn indirect_with_unset_name_errors() {
    // bash: `undefined: invalid indirect expansion`, rc 1.
    let (_out, err, code) = run(r#"echo "[${!undefined}]""#);
    assert_eq!(code, 1, "stderr={err}");
    assert!(
        err.contains("undefined: invalid indirect expansion"),
        "stderr={err}"
    );
}

#[test]
fn indirect_name_bound_to_empty_errors() {
    // bash reports `: invalid variable name` (same rc); fdshell keeps the rc
    // and names the empty name in its own message.
    let (_out, err, code) = run(r#"p=""; echo "[${!p}]""#);
    assert_eq!(code, 1, "stderr={err}");
    assert!(err.contains(": invalid indirect expansion"), "stderr={err}");
}

#[test]
fn indirect_target_bound_to_empty_is_empty() {
    // `empty` is a real name bound to the empty string: length 0, not invalid.
    let (out, err, code) = run(r#"empty=""; e=empty; echo "[${!e}]""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[]\n");
}

// --- `set -u` covers every parameter arm ---

#[test]
fn nounset_braced_var_fails() {
    let (_out, err, code) = run(r#"set -u; echo "[${x}]""#);
    assert_eq!(code, 1, "stderr={err}");
    assert!(err.contains("x: unbound variable"), "stderr={err}");
}

#[test]
fn nounset_braced_length_fails() {
    let (_out, err, code) = run(r#"set -u; echo "[${#x}]""#);
    assert_eq!(code, 1, "stderr={err}");
    assert!(err.contains("x: unbound variable"), "stderr={err}");
}

#[test]
fn nounset_indirect_unbound_target_fails() {
    // bash's message carries the whole body: `!q: unbound variable`.
    let (_out, err, code) = run(r#"set -u; q=missing; echo "[${!q}]""#);
    assert_eq!(code, 1, "stderr={err}");
    assert!(err.contains("!q: unbound variable"), "stderr={err}");
}

#[test]
fn nounset_off_restores_empty_for_every_arm() {
    let (out, _err, code) = run(r#"set -u; set +u; echo "[${x}]""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[]\n");
    let (out, _err, code) = run(r#"set -u; set +u; echo "${#x}""#);
    assert_eq!(code, 0);
    assert_eq!(out, "0\n");
}

// --- positionals and word splitting are unchanged by the rule ---

#[test]
fn positional_concatenation_drops_out_of_range() {
    // `$1..` are the `set --` words in order, so `$3` is the out-of-range one:
    // out-of-range `$N` expands to empty on both, and the line matches bash.
    let (out, _err, code) = run(r#"set -- a b; echo "[$1$2$3]""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[ab]\n");
}

#[test]
fn unset_word_drops_in_a_test() {
    // The unquoted word vanishes. `test -z` sees no argument (rc 0); `test -n`
    // sees the single word `-n`, which is a true non-empty string test (rc 0),
    // as in bash. A quoted unset word survives as one empty word, so `-n` is
    // false (rc 1) — bash-identical.
    let (out, _err, code) = run("[ -z $unset ]; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "0\n");
    let (out, _err, code) = run("[ -n $unset ]; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "0\n");
    let (out, _err, code) = run(r#"[ -n "$unset" ]; echo $?"#);
    assert_eq!(code, 0);
    assert_eq!(out, "1\n");
}

// --- accepted divergences: malformed braces stay literal ---

#[test]
fn empty_brace_name_stays_literal() {
    // bash rejects `${}` (`bad substitution`, rc 1); fdshell keeps it literal.
    let (out, _err, code) = run(r#"echo "[${}]""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[${}]\n");
}

#[test]
fn empty_indirect_name_stays_literal() {
    // `${!}` has an empty indirect name. Bash expands it to empty (`[]`, rc 0)
    // and `[${!}x]` to `[x]`; fdshell keeps the braced text literal at rc 0,
    // like the `${}` divergence.
    let (out, err, code) = run(r#"echo "[${!}]""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[${!}]\n");
    let (out, err, code) = run(r#"echo "[${!}x]""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[${!}x]\n");
}

#[test]
fn unclosed_brace_stays_literal() {
    // bash is a parse error (rc 2); fdshell keeps the text literal.
    let (out, _err, code) = run(r#"echo "[${x]""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[${x]\n");
}

#[test]
fn escaped_dollar_stays_literal() {
    // The escape path is not a parameter substitution: an escaped `$` is
    // literal in bash too.
    let (out, _err, code) = run(r#"echo "x\$unset""#);
    assert_eq!(code, 0);
    assert_eq!(out, "x$unset\n");
}

#[test]
fn assignment_of_unset_uses_the_default_form() {
    // `x` becomes empty (not unset), so `${x:-unset}` prints the default.
    let (out, _err, code) = run(r#"x=$unset; echo "[${x:-unset}]""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[unset]\n");
}

// Task #130: a `${ … }` region inside double quotes is one unit, so its inner
// quotes do not close the outer quote. The default value keeps its spaces
// (bash parity), the word stays one word, and the `;`/`&&` inside the region
// are data.
#[test]
fn quoted_brace_default_value_matches_bash() {
    let cases: &[(&str, &str)] = &[
        (r#"u=""; echo "[${u:-"z w"}]""#, "[z w]\n"),
        (r#"u=""; builtin printf "[%s]" "${u:-"z w"}""#, "[z w]"),
        (r#"echo "${u:-"a; b"}""#, "a; b\n"),
        (r#"echo "${u:-"a && b"}" && echo ok"#, "a && b\nok\n"),
        (
            r#"if echo "${u:-"a && b"}"; then echo yes; fi"#,
            "a && b\nyes\n",
        ),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(err, "", "{script:?} must report nothing, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}
/// Accepted divergence (owner task #174): the substitution pass takes a `${ … }`
/// body verbatim, so a nested `$( … )` inside it is *scanned* as one unit by
/// #130 (one word, exit 0, no parse error) but is not expanded.
#[test]
fn nested_paren_inside_a_brace_region_is_scanned_but_not_expanded() {
    let (out, err, code) = run(r#"echo "x${u:-"$(echo "p q")"}y""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "x$(echo p q)y\n");
}
