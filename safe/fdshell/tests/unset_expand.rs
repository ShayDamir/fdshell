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
/// Task #561 moves this row to bash parity: the word of a colon operator is a
/// word, so the nested `$( … )` — scanned as one unit by #130 (one word, exit 0,
/// no parse error) — now runs as part of the word's expansion. bash prints
/// `xp qy` for the same script.
#[test]
fn nested_paren_inside_a_brace_region_is_expanded() {
    let (out, err, code) = run(r#"echo "x${u:-"$(echo "p q")"}y""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "xp qy\n");
}

// Accepted divergence (task #172): an indirect target that is not a valid
// variable name expands to empty here. bash rejects the name and prints
// nothing: `a b: invalid variable name` / `a-b: invalid variable name`, rc 1.
#[test]
fn indirect_target_that_is_not_a_name_is_empty() {
    for script in [r#"p="a b"; echo "[${!p}]""#, r#"p=a-b; echo "[${!p}]""#] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must expand quietly, stderr={err}");
        assert_eq!(out, "[]\n", "{script:?}");
        assert_eq!(err, "", "{script:?} must report nothing");
    }
}

// Accepted divergence (task #172): bash resolves `-` to its special `$-`
// parameter (`[hBc]`, rc 0). fdshell's indirect target keeps the plain-name
// lookup (task #177's D4 rule, owned by task #260) and looks for a variable
// literally named `-`: unbound, so the expansion is empty.
#[test]
fn indirect_target_that_is_the_dash_name_is_empty() {
    let (out, err, code) = run(r#"p="-"; echo "[${!p}]""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[]\n");
    assert_eq!(err, "");
}

// --- task #176: `${!name}` inside the colon operators ---

#[test]
fn indirect_with_colon_operators() {
    // The reference resolves to the target's value; only `:+` takes the word
    // (its target is set and non-empty) and `:=` has nothing to write, so `x`
    // keeps `abcabc`. Measured against bash 5.3.9.
    let (out, err, code) = run(
        r#"x=abcabc; v=x; r0=${!v:-z}; r1=${!v:+z}; r2=${!v:?m}; r3=${!v:=q}; r4=${!v}; printf "[%s]" "$r0" "$r1" "$r2" "$r3" "$r4"; printf "|x=%s" "$x""#,
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[abcabc][z][abcabc][abcabc][abcabc]|x=abcabc");
}

#[test]
fn indirect_assign_writes_the_indirect_target() {
    // `:=` writes the parameter the reference points at, never the `!` body:
    // an empty target is filled, an unset one is created, a set one is left
    // alone, and the reference variable keeps its own value in all three.
    let cases: &[(&str, &str)] = &[
        (
            r#"x=; v=x; r=${!v:=q}; printf "[%s][%s][%s]" "$r" "$x" "$v""#,
            "[q][q][x]",
        ),
        (
            r#"v=y; r=${!v:=q}; printf "[%s][%s][%s]" "$r" "$y" "$v""#,
            "[q][q][y]",
        ),
        (
            r#"x=abc; v=x; r=${!v:=q}; printf "[%s][%s][%s]" "$r" "$x" "$v""#,
            "[abc][abc][x]",
        ),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must assign, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}

#[test]
fn indirect_colon_family_stays_nounset_exempt() {
    // The colon family carries its own word, so `set -u` never bails: an unset
    // target yields the word, and `:=` assigns at rc 0. The pattern family
    // bails on the same input (`nounset_bails_a_pattern_parameter`).
    let cases: &[(&str, &str)] = &[
        (r#"set -u; v=undef_target; printf "[%s]" "${!v:-z}""#, "[z]"),
        (
            r#"set -u; x=; v=x; printf "[%s][%s]" "${!v:-z}" "${!v:+z}""#,
            "[z][]",
        ),
        (
            r#"set -u; x=; v=x; r=${!v:=q}; printf "[%s][%s]" "$r" "$x""#,
            "[q][q]",
        ),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must expand, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}

#[test]
fn indirect_colon_unresolvable_reference_errors() {
    // An unbound name, or a name bound to the empty string, names no target:
    // the word does not rescue it. bash: `undef: invalid indirect expansion` /
    // `: invalid variable name`, rc 1 (bash `-c` prints 127, the script rc is 1).
    let bails: &[(&str, &str)] = &[
        (
            r#"printf "[%s]" "${!undef:-z}""#,
            "undef: invalid indirect expansion",
        ),
        (
            r#"set -u; printf "[%s]" "${!undef:-z}""#,
            "undef: invalid indirect expansion",
        ),
        (
            r#"p=""; printf "[%s]" "${!p:-z}""#,
            ": invalid indirect expansion",
        ),
        (
            r#"set -u; p=""; printf "[%s]" "${!p:-z}""#,
            ": invalid indirect expansion",
        ),
    ];
    for (script, want) in bails {
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "{script:?} must bail, stderr={err}");
        assert_eq!(out, "", "{script:?} must print nothing");
        assert!(err.contains(want), "{script:?} stderr={err}");
    }
    // `:=` resolves its write target first, so it bails and assigns nothing.
    for script in [
        r#"p=""; r=${!p:=q}; printf "never""#,
        r#"set -u; p=""; r=${!p:=q}; printf "never""#,
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "{script:?} must bail, stderr={err}");
        assert_eq!(
            out, "",
            "{script:?} must print nothing, so nothing was assigned"
        );
    }
}

// Accepted divergence (#172 family): an indirect target value that is not a
// valid name is just an unset parameter for the colon family, so its word
// supplies the value at rc 0. bash rejects the name (`a b: invalid variable
// name`, `a-b: invalid variable name`), rc 1.
#[test]
fn indirect_target_that_is_not_a_name_with_a_word() {
    for script in [
        r#"p="a b"; printf "[%s]" "${!p:-z}""#,
        r#"p=a-b; printf "[%s]" "${!p:-z}""#,
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must expand quietly, stderr={err}");
        assert_eq!(out, "[z]", "{script:?}");
        assert_eq!(err, "", "{script:?} must report nothing");
    }
}

#[test]
fn degenerate_empty_indirect_name_with_colon_operators() {
    // The scan name is exactly `!`: bash treats it as unset (`:-` takes the
    // word, `:+` prints nothing), with and without `set -u`.
    for script in [
        r#"printf "[%s][%s]" "${!:-z}" "${!:+z}""#,
        r#"set -u; printf "[%s][%s]" "${!:-z}" "${!:+z}""#,
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must expand, stderr={err}");
        assert_eq!(&out, "[z][]", "{script:?}");
        assert_eq!(err, "", "{script:?} must report nothing");
    }
    // Accepted divergence: bash rejects the whole form (`${!?m}: bad
    // substitution`, rc 1; `$!: cannot assign in this way`, rc 1). fdshell
    // keeps the rc for `:?` and assigns at rc 0 for `:=`.
    let (out, err, code) = run(r#"printf "[%s]" "${!?m}""#);
    assert_eq!(code, 1, "stderr={err}");
    assert_eq!(out, "");
    assert!(
        err.contains("?m: invalid indirect expansion"),
        "stderr={err}"
    );
    let (out, err, code) = run(r#"r=${!:=z}; printf "[%s]" "$r""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[z]");
}

// Accepted divergence (task #177, the write face of the #172/#260 indirect
// target rule): `:=` resolves `!name` to the parameter its value names and then
// refuses to assign it, so it bails at rc 1 and writes nothing where bash has
// the parameter's value at rc 0 — `p=1` prints `[a]`, `p="-"` prints `[hBc]`,
// `p="#"` prints `[0]`; `p="@"` is rc 1 in bash too (`@: invalid variable
// name`), so only the wording differs there. Task #260 owns resolving the
// target as a parameter.
#[test]
fn indirect_target_naming_a_parameter_refuses_assign() {
    for (script, want) in [
        (
            r#"set -- a b c; p=1; printf "[%s]" "${!p:=q}""#,
            "1: cannot assign in this way",
        ),
        (
            r#"set -- a b c; p=1; set -u; printf "[%s]" "${!p:=q}""#,
            "1: cannot assign in this way",
        ),
        (
            r#"p="-"; printf "[%s]" "${!p:=q}""#,
            "-: cannot assign in this way",
        ),
        (
            r##"p="#"; printf "[%s]" "${!p:=q}""##,
            "#: cannot assign in this way",
        ),
        (
            r#"p="@"; printf "[%s]" "${!p:=q}""#,
            "@: cannot assign in this way",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "{script:?} must bail, stderr={err}");
        assert_eq!(
            out, "",
            "{script:?} must print nothing, so nothing was assigned"
        );
        assert!(err.contains(want), "{script:?} stderr={err}");
    }
    // The read face of the same target: the target is an unset *variable*, so
    // the colon family's word supplies the value where bash has the positional
    // (`${!p:-z}` → `[z]` here, `[a]` in bash; `:+` prints nothing here).
    let (out, err, code) = run(r#"set -- a b c; p=1; printf "[%s][%s]" "${!p:-z}" "${!p:+z}""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[z][]", "the colon word supplies the unset target");
}

// --- task #177: positional and special parameters in the shared lookup ---

/// `set -u` bails an out-of-range positional in the plain and pattern families
/// and names the index, exactly as bash does; the colon family keeps its
/// exemption because its own word supplies the value.
#[test]
fn braced_positional_family_nounset() {
    for (script, want) in [
        (
            r#"set -- a; set -u; printf "[%s]" "${2}""#,
            "2: unbound variable",
        ),
        (
            r#"set -- a; set -u; printf "[%s]" "${2#x}""#,
            "2: unbound variable",
        ),
        (
            r#"set -- a; set -u; printf "[%s]" "${2:?}""#,
            "2: parameter null or not set",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "{script:?} must bail, stderr={err}");
        assert_eq!(out, "", "{script:?} must print nothing");
        assert!(err.contains(want), "{script:?} stderr={err}");
    }
    for (script, want) in [
        (r#"set -- a; set -u; printf "[%s]" "${2:-z}""#, "[z]"),
        (r#"set -- a; set -u; printf "[%s]" "${2:+p}""#, "[]"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must expand, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}

/// `$0` and every single-byte special are always bound: `set -u` never reports
/// them, and the expansion is their value (bash: `huBc` for `${-}`; fdshell
/// renders its own option flags, empty by default — divergence D2).
#[test]
fn braced_specials_are_never_unbound() {
    let (out, err, code) = run(r#"set --; printf "[%s]" "${0}" "${?}" "${@}" "${*}" "${-}""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[sh][0][][][]");
    let (out, err, code) =
        run(r#"set --; set -u; printf "[%s]" "${0}" "${?}" "${@}" "${*}" "${-}""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[sh][0][][][u]");
    // Divergence D2's `:?` face: fdshell's flag bytes are empty by default, so
    // `${-}` is an empty parameter and `:?` bails at rc 1 where bash (whose
    // `$-` is `hBc`) prints its flags at rc 0.
    let (out, err, code) = run(r#"printf "[%s]" "${-:?m}""#);
    assert_eq!(code, 1, "stderr={err}");
    assert_eq!(out, "", "the bail prints nothing");
    assert!(err.contains("-: m"), "stderr={err}");
}

/// Accepted divergences (task #177), each pinned with its rc:
/// * D3 — bash prefixes the assignment message with `$` (`$5: cannot assign in
///   this way`), fdshell names the parameter (`5: cannot assign in this way`).
/// * D5 — a digit run too large for `usize` keeps fdshell's `TooLarge` bail
///   (rc 1) in the braced form, where bash expands it to empty at rc 0.
/// * D4 — an indirect target keeps the plain-name lookup: bash resolves `1` to
///   the positional, fdshell expands to empty (task #260).
#[test]
fn braced_parameter_assignment_divergences() {
    let (out, err, code) = run(r#"set -- a b c; r=${5:=q}; printf "never""#);
    assert_eq!(out, "", "stderr={err}");
    assert_eq!(code, 1, "stderr={err}");
    assert!(err.contains("5: cannot assign in this way"), "stderr={err}");
    let (out, err, code) = run(r#"set -- a b c; set -u; printf "[%s]" "${99999999999999999999}""#);
    assert_eq!(out, "", "stderr={err}");
    assert_eq!(code, 1, "stderr={err}");
    assert!(
        err.contains("index or value too large for type"),
        "stderr={err}"
    );
    let (out, err, code) = run(r#"set -- a b c; v=1; r=${!v}; printf "[%s]" "$r""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[]");
}

// --- task #561: the word of a colon operator goes through the word pipeline ---

/// Every expectation is bash 5.3.9's own output for the same script (`HOME` is
/// set inside the script so no row depends on the ambient home). The word of
/// `:-` / `:=` / `:+` / `:?` goes through the ordinary pipeline — command
/// substitution, parameter and arithmetic expansion, tilde, escapes — and the
/// word's own quote bits govern its expansion: `"${u:=a\ b}"` keeps the
/// backslash (POSIX #2.2.2), `"${u:=~/x}"` keeps the tilde literal, while the
/// unquoted spellings fold the escape and expand the tilde. `:?` expands its
/// message word **unquoted** even inside a quoted reference, so its rows show
/// the folded and expanded forms.
#[test]
fn colon_word_expansion_matches_bash() {
    for (script, want) in [
        (r#"u=; printf "%s" "${u:=$(printf hi)}""#, "hi"),
        (r#"u=; printf "%s" ${u:=$(printf hi)}"#, "hi"),
        (r#"v=V; u=; printf "%s" "${u:=$v}""#, "V"),
        (r#"u=; printf "%s" "${u:=$((1+2))}""#, "3"),
        (r#"u=; v=V; printf "%s" "${u:-$(printf hi)}""#, "hi"),
        (r#"v=V; u=; printf "%s" "${v:-$(printf hi)}""#, "V"),
        (r#"v=V; u=; printf "%s" "${v:+$(printf hi)}""#, "hi"),
        (r#"u=; v=V; printf "%s" "${u:+$(printf hi)}""#, ""),
        (r#"u=; printf "%s" "${u:=a\ b}""#, "a\\ b"),
        (r#"HOME=/h; u=; printf "%s" "${u:=~/x}""#, "~/x"),
        (r#"HOME=/h; u=; printf "%s" ${u:=~/x}"#, "/h/x"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script:?}: stderr={err}");
    }
    for (script, want) in [
        (r#"u=; printf "%s" "${u:?$(printf boom)}""#, "u: boom"),
        (r#"HOME=/h; u=; printf "%s" "${u:?~}""#, "u: /h"),
        (r#"u=; printf "%s" "${u:?a\ b}""#, "u: a b"),
        (r#"u=; printf "%s" ${u:?""}"#, "u: "),
        (r#"HOME=/h; u=; printf "%s" ${u:?~""}"#, "u: ~"),
        (r#"u=; printf "%s" "${u:?$(true)}""#, "u: "),
    ] {
        // bash exits 127 with the same message; fdshell bails at its own status.
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "{script:?} must bail, stderr={err}");
        assert_eq!(out, "", "{script:?} must print nothing");
        assert!(err.contains(want), "{script:?} stderr={err}");
    }
    // The one core row that stays divergent: a **quoted** space inside a
    // substitution written in a region body. bash has one word `a b`; the parser
    // strips the region body's quote bytes (documented divergence, pinned by
    // `tests/quotes.rs::region_body_drops_the_span_quote_bytes`), so the child
    // runs `printf a b` and only the first line comes back.
    let (out, err, code) = run(r#"u=; printf "%s" "${u:=$(printf "a b")}""#);
    assert_eq!((out.as_str(), code), ("a", 0), "stderr={err}");
}

/// `:=` stores the **expanded** word, so the parameter and the reference agree
/// (bash's values; `HOME` is set inside the script). Persistence is pinned inside
/// one command: `:=` across separate commands is task #153.
#[test]
fn colon_assign_stores_the_expanded_scalar() {
    for (script, want) in [
        (
            r#"u=; w=${u:=$(printf hi)}; printf "%s %s" "$u" "$w""#,
            "hi hi",
        ),
        (
            r#"HOME=/h; u=; w=${u:=~/x}; printf "%s %s" "$u" "$w""#,
            "/h/x /h/x",
        ),
        (
            r#"u=; w=${u:=a\ b}; printf "<%s><%s>" "$u" "$w""#,
            "<a b><a b>",
        ),
        (r#"u=; v=V; w=${u:=$v}; printf "%s %s" "$u" "$w""#, "V V"),
        (
            r#"u=; w=${u:='$v'}; printf "<%s> <%s>" "$u" "$w""#,
            "<$v> <$v>",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script:?}: stderr={err}");
    }
    // The cross-command row, pinned as the divergence it is: bash has `x` in the
    // second `printf`, fdshell an empty `u` (task #153 — each `printf` is a
    // forked child that loses the assignment).
    let (out, err, code) = run(r#"u=; printf "%s" "${u:=x}"; printf "u=%s" "$u""#);
    assert_eq!((out.as_str(), code), ("xu=", 0), "stderr={err}");
}

/// bash does not expand the word of an arm that does not take it, so `set -u`
/// never sees an unbound name inside it; when the arm does take the word, the
/// word's own name bails. bash reports the same messages and exits 127, fdshell
/// bails at its own status (task #177's convention).
#[test]
fn colon_word_expansion_is_lazy_under_nounset() {
    for (script, want) in [
        (r#"set -u; v=V; printf "%s" "${v:-$nope}""#, "V"),
        (r#"set -u; u=; printf "%s" "${u:+$nope}""#, ""),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script:?}: stderr={err}");
    }
    for (script, want) in [
        (
            r#"set -u; u=; printf "%s" "${u:-$nope}""#,
            "nope: unbound variable",
        ),
        (
            r#"set -u; u=; printf "%s" "${u:?$nope}""#,
            "nope: unbound variable",
        ),
        (r#"set -u; printf "%s" "${u:?boom}""#, "u: boom"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "{script:?} must bail, stderr={err}");
        assert_eq!(out, "", "{script:?} must print nothing");
        assert!(err.contains(want), "{script:?} stderr={err}");
    }
}

/// A command substitution in the word runs, so it leaves its status behind for
/// the rest of the word. bash's values, rc 0 in both shells (`false` resolves to
/// a program that exits 0 in this environment, which is why the last row is
/// `<0> rc=0` in both).
#[test]
fn colon_word_command_substitution_updates_dollar_question() {
    for (script, want) in [
        (
            r#"u=; printf "<%s> rc=%s" "${u:=$(false)}" "$?""#,
            "<> rc=1",
        ),
        (
            r#"u=; printf "<%s> rc=%s" "${u:=$(exit 3)}" "$?""#,
            "<> rc=3",
        ),
        (
            r#"v=V; u=; printf "<%s> rc=%s" "${v:+$(exit 3)}" "$?""#,
            "<> rc=3",
        ),
        (
            r#"false; u=; printf "<%s> rc=%s" "${u:=$?}" "$?""#,
            "<0> rc=0",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script:?}: stderr={err}");
    }
}

/// `set -- a b` in bash. `:-` keeps the argument join, so a quoted word still
/// cuts two words and an unquoted one splits after the IFS join; `:=` joins into
/// the one scalar it stores; the `:?` message space-joins whatever the `IFS` is.
/// Every row matches bash.
#[test]
fn colon_word_positional_join_rows() {
    for (script, want) in [
        (r#"set -- a b; u=; printf "<%s>" "${u:-$@}""#, "<a><b>"),
        (r#"set -- a b; u=; printf "<%s>" ${u:-$@}"#, "<a><b>"),
        (r#"set -- a b; u=; printf "<%s>" "${u:=$@}""#, "<a b>"),
        (r#"set -- a b; u=; printf "<%s>" ${u:=$@}"#, "<a><b>"),
        (
            r#"set -- a b; u=; printf "%s|%s" "${u:=$@}" "$u""#,
            "a b|a b",
        ),
        (
            r#"set -- a b; IFS=,; u=; printf "<%s>" "${u:=$*}""#,
            "<a,b>",
        ),
        (
            r#"set -- a b; IFS=,; u=; printf "<%s>" "${u:-$*}""#,
            "<a,b>",
        ),
        (r#"set -- a b; u=; printf "<%s>" "${u:-$*}""#, "<a b>"),
        (r#"set -- a ""; u=; printf "<%s>" "${u:-$@}""#, "<a><>"),
        (r#"set -- a ""; u=; printf "<%s>" "${u:=$@}""#, "<a >"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script:?}: stderr={err}");
    }
    for (script, want) in [
        (r#"set -- a b; u=; printf "%s" "${u:?$@}""#, "u: a b"),
        (r#"set -- a b; IFS=,; u=; printf "%s" "${u:?$*}""#, "u: a b"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "{script:?} must bail, stderr={err}");
        assert_eq!(out, "", "{script:?} must print nothing");
        assert!(err.contains(want), "{script:?} stderr={err}");
    }
}

/// The region body is still read to the first `}` (task #174), so the word of
/// this row is a fragment; the truncation guard keeps it literal and the row
/// stays at rc 0 instead of becoming an `UnclosedParen` bail. bash runs the
/// substitution and prints `}`; fdshell's current literal is pinned.
#[test]
fn colon_word_truncated_nested_substitution_stays_literal() {
    let (out, err, code) = run(r#"u=; printf "<%s>" "${u:=$(echo })}""#);
    assert_eq!((out.as_str(), code), ("<$(echo )}>", 0), "stderr={err}");
    // The backtick analogue cannot stay literal end to end: the whole quoted word
    // reaches the substitution pass, so after the region ends at the `}` the
    // word's own trailing backtick is an unclosed command substitution (bash runs
    // `` `echo ` `` there and prints `}`). The guard on the word itself is pinned
    // by `substitute::tests::colon_word_truncated_paren_stays_literal`.
    let (out, err, code) = run(r#"u=; printf "<%s>" "${u:=`echo }`}""#);
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err}");
    assert!(err.contains("unclosed backtick"), "stderr={err}");
}

/// The unquoted-word row is the tokenizer's gap, not the substitution's:
/// `parse/token/step.rs` does not treat an unquoted `${…}` as one unit, so this
/// word is two tokens and each keeps its own literal (task #174's body scanner
/// owns it; bash has one word `a b`). The space-free spelling below is parity.
#[test]
fn colon_word_unquoted_space_row_is_still_the_tokenizer_gap() {
    let (out, err, code) = run(r#"u=; printf "<%s>" ${u:=a b}"#);
    assert_eq!((out.as_str(), code), ("<${u:=a><b}>", 0), "stderr={err}");
    let (out, err, code) = run(r#"HOME=/h; u=; printf "<%s>" ${u:=~/x}"#);
    assert_eq!((out.as_str(), code), ("</h/x>", 0), "stderr={err}");
}
