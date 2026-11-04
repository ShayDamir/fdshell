#![allow(clippy::unwrap_used)]

//! The shell array assignment `name=(words)` and the element reads (task #422):
//! the region is assembled at the token level (`parse/array_literal.rs`), the
//! elements live in their own store (`state/shell_arrays.rs`), and the reads are
//! the array twins of the positional machinery
//! (`substitute/array_fields.rs` — whole-word fast path and embedded arm, which
//! LESSONS "`$@` has two join rules" requires to agree).
//!
//! Every bash column below was measured with `bash -c` (5.3.9) and every fdshell
//! column with `target/debug/fdshell -c` on this task's branch before the rows
//! were pinned. The `set -u` rows keep fdshell's own report and rc (the
//! rc-1-vs-127 family, `tests/positional.rs`); the `Pinned as measured` test
//! holds the rows where the two differ, each with bash's answer in the comment.
//!
//! ```text
//! m=(1); echo "${m[@]}"                    bash `1`      fdshell `1`      ← the task's symptom
//! case x in x) m=(1); echo done ;; esac    bash `done`   fdshell `done`   (+ clean stderr)
//! x=(1 2); echo "${x[1]}"                  bash `2`      fdshell `2`
//! m=(); echo "${#m[@]}"                    bash `0`      fdshell `0`
//! a=("b c" d); printf "[%s]" "${a[@]}"     bash `[b c][d]`                ← two fields
//! a=("b c" d); printf "[%s]" "x ${a[@]} y" bash `[x b c][d y]`            ← the boundary model
//! a=(); printf "[%s]" "${a[@]}"; echo      bash `[]`                      ← no field at all (#219)
//! a=hello; echo "${a[0]}"                  bash `hello`                   ← the scalar is index 0
//! a=(1 2)y; echo "[${a[@]}]"            bash `[(1 2)y]` fdshell `[]` + a bogus-command report ← the region ends at `)`
//! ```

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

/// bash-parity row: rc 0, nothing on stderr, and this stdout.
fn check(script: &str, want: &str) {
    let (out, err, code) = run(script);
    assert_eq!(code, 0, "{script:?} must succeed, stderr={err:?}");
    assert_eq!(err, "", "{script:?} must stay silent");
    assert_eq!(out, want, "{script:?}");
}

/// The task's four pinned rows.
#[test]
fn the_literal_stops_being_a_command() {
    // `m=(1)` used to run the tokenizer's `)` as a command (`")` not found`).
    check(r#"m=(1); echo "${m[@]}""#, "1\n");
}

#[test]
fn a_case_arm_run_keeps_its_stderr_clean() {
    // Task #300's parse rule plus this task's run: the arm's `m=(1)` is an
    // assignment, so the arm prints `done` and nothing else (bash, rc 0).
    let (out, err, code) = run(r#"case x in x) m=(1); echo done ;; esac"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(err, "", "the bogus `)`-command report is the symptom");
    assert_eq!(out, "done\n");
}

#[test]
fn a_bracket_read_is_an_element() {
    check(r#"x=(1 2); echo "${x[1]}""#, "2\n");
}

#[test]
fn an_empty_array_has_no_elements() {
    // `m=()` used to be a parse error (`'m=()' is missing its closing '}`).
    check(r#"m=(); echo "${#m[@]}""#, "0\n");
}

/// The whole-word fast path in `substitute/fields.rs` and the embedded arm in
/// `substitute/brace.rs`, pinned side by side (the LESSONS rule for the
/// positional pair applies verbatim).
#[test]
fn the_whole_word_and_embedded_arms_agree() {
    let cases: &[(&str, &str)] = &[
        (r#"a=(1 2); printf "[%s]\n" "${a[@]}"#, "[1]\n[2]\n"),
        (r#"a=("b c" d); printf "[%s]" "${a[@]}"; echo"#, "[b c][d]"),
        (r#"a=("b c" d); printf "[%s]" ${a[@]}; echo"#, "[b][c][d]"),
        (r#"a=("b c" d); printf "[%s]" "${a[*]}"; echo"#, "[b c d]"),
        (r#"a=("b c" d); printf "[%s]" "x ${a[@]} y"; echo"#, "[x b c][d y]"),
        (r#"a=("b c" d); printf "[%s]" "x ${a[*]} y"; echo"#, "[x b c d y]"),
        // The empty-array contract is the positional one (task #219).
        (r#"a=(); printf "[%s]" "${a[@]}"; echo"#, "[]"),
        (r#"a=(); echo "x ${a[*]} y""#, "x  y\n"),
        // The glue of a quoted arm stays with the element it touches.
        (r#"a=(b c); printf "[%s]" "${a[@]}x"; echo"#, "[b][cx]"),
        (r#"a=(b c); printf "[%s]" "x${a[@]}"; echo"#, "[xb][c]"),
        (r#"a=(b c); printf "[%s]" x"${a[@]}"; echo"#, "[xb][c]"),
        (r#"a=(b ""); printf "[%s]" "${a[@]}"; echo"#, "[b][]"),
        (r#"a=(""); printf "[%s]" "x${a[@]}y"; echo"#, "[xy]"),
        // `IFS` decides the `[*]` join: the first byte, or nothing at all.
        (r#"IFS=; a=("b c" d); printf "[%s]" "${a[*]}"; echo"#, "[b cd]"),
        (r#"IFS=,; a=("b c" d); printf "[%s]" "x ${a[*]} y"; echo"#, "[x b c,d y]"),
        // Two arms in one word: the fast path claims one substitution only.
        (r#"a=(b c); echo "${a[@]}${a[@]}""#, "b cb c\n"),
    ];
    for (script, want) in cases {
        check(script, want);
    }
}

#[test]
fn for_and_set_take_the_element_list() {
    let cases: &[(&str, &str)] = &[
        (
            r#"m=(a "b c"); for i in "${m[@]}"; do echo "[$i]"; done"#,
            "[a]\n[b c]\n",
        ),
        (
            r#"a=("b c" d); for x in ${a[@]}; do echo "[$x]"; done"#,
            "[b]\n[c]\n[d]\n",
        ),
        (
            r#"a=("b c" d); for x in "${a[@]}"; do echo "[$x]"; done"#,
            "[b c]\n[d]\n",
        ),
        (r#"a=(1 2); set -- "${a[@]}"; echo "$#""#, "2\n"),
    ];
    for (script, want) in cases {
        check(script, want);
    }
}

/// The writers: `name=(…)`, `name+=(…)`, `name[idx]=value`, a plain write to a
/// name the store holds, and `unset` — and the one-namespace rule that puts the
/// scalar at index 0 (`state/shell_arrays.rs`).
#[test]
fn the_writers_and_the_one_namespace_rule() {
    let cases: &[(&str, &str)] = &[
        (r#"m=(1); m+=(3); echo "${m[@]}""#, "1 3\n"),
        (r#"a=(1 2); a+=(); echo "${#a[@]}""#, "2\n"),
        (r#"a[1]=x; echo "${a[1]}|${a[@]}|${#a[@]}""#, "x|x|1\n"),
        (r#"a=(p q); a[1]=z; echo "${a[@]}""#, "p z\n"),
        (r#"a=1; a[0]=x; echo "${a[@]}""#, "x\n"),
        (r#"a=(1 2); a=scalar; echo "${a[@]}|${a}""#, "scalar 2|scalar\n"),
        (r#"a=(1 2); a[5]=x; printf "[%s]" "${a[@]}"; echo "${#a[@]}""#, "[1][2][x]3\n"),
        (r#"a=(1) b=(2); echo "${a[@]}-${b[@]}""#, "1-2\n"),
        (r#"a=hello; echo "${a[0]}|${a[1]}""#, "hello|\n"),
        (r#"a=hello; echo "${#a[@]}""#, "1\n"),
        (r#"a=hello; a+=(x); echo "${a[@]}""#, "hello x\n"),
        (r#"a=(1 2); echo "$a""#, "1\n"),
        (r#"a=(1 2); unset a; echo "${#a[@]}""#, "0\n"),
        (r#"a=(1 2); unset a; echo "[${a[@]}]""#, "[]\n"),
    ];
    for (script, want) in cases {
        check(script, want);
    }
}

/// Each element word goes through the ordinary per-word pipeline (substitution,
/// `IFS` splitting, glob), so bash's element semantics come free.
#[test]
fn every_element_word_is_expanded_on_its_own() {
    let cases: &[(&str, &str)] = &[
        (r#"a=($(echo 1 2)); echo "${#a[@]}""#, "2\n"),
        (r#"a=("$(echo hi)"); echo "${a[@]}""#, "hi\n"),
        (r#"a=("*"); echo "${#a[@]}""#, "1\n"),
        (r#"a=(*.nonexistent); echo "${a[@]}""#, "*.nonexistent\n"),
        (r#"a=("x y"); printf "[%s]" "${a[@]}"; echo"#, "[x y]"),
        (r#"a=("b c" d); echo "${#a} ${#a[1]}""#, "3 1\n"),
        (r#"a=(1 2); echo "${a[0]}-${a[1]}-${a[2]}""#, "1-2-\n"),
        (r#"a=(1 2); echo "${#a[0]}${#a[1]}${#a[2]}${#a[@]}""#, "1102\n"),
        (r#"a=(1 2); echo "${a[2]}${a[9]}x""#, "x\n"),
    ];
    for (script, want) in cases {
        check(script, want);
    }
}

#[test]
fn nounset_keeps_the_list_reads_and_bails_on_an_element() {
    // bash 5.3.9: an unset name's `[@]`/`[*]` expands empty at rc 0, while an
    // out-of-range element bails — fdshell's report and rc are its own (the
    // rc-1-vs-127 family), the message is the same.
    check(r#"set -u; echo "${q[@]}""#, "\n");
    check(r#"set -u; echo "${q[*]}""#, "\n");
    check(r#"set -u; q=(); echo "${q[@]}""#, "\n");
    check(r#"set -u; a=(1); echo "${a[@]} ${a[*]} ${#a[@]}""#, "1 1 1\n");
    let (out, err, code) = run(r#"set -u; a=(1); echo "${a[3]}""#);
    assert_eq!(out, "");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("a[3]: unbound variable"), "stderr={err:?}");
}

/// The neighbours the task must not move: task #300's `case`-arm pins, the
/// arithmetic and command-substitution regions the tokenizer keeps whole, and
/// the `%` namespace that owns fd-valued arrays (README "Variables").
#[test]
fn the_neighbours_stay_green() {
    let cases: &[(&str, &str)] = &[
        (r#"case a in a) ((1)) ;; esac"#, ""),
        (r#"case x in a) m=(1) ;; esac"#, ""),
        (
            r#"case x in x) for ((i=0;i<2;i++)); do echo $i; done;; esac"#,
            "0\n1\n",
        ),
        (r#"m=$((1)); echo "ok $m""#, "ok 1\n"),
        (r#"echo $((1+1))"#, "2\n"),
        (r#"a=(1 2); echo "[%a]""#, "[%a]\n"),
        (r#"a=(1 2); export a; echo "$a""#, "1\n"),
        (r#"a=(1 2); export a; sh -c 'echo "[child $a]"'"#, "[child ]\n"),
    ];
    for (script, want) in cases {
        check(script, want);
    }
}

/// Pinned **as measured** (fdshell's answer; bash's in the comment, so the
/// divergence is on record). All of them are the tokenizer's segmentation, the
/// `set -u` reporting convention, and the bash features task #422 leaves to the
/// follow-up task (scoped arrays/`local`, slices, `${!name[@]}`, negative
/// indices, `declare`, scalar `+=`).
#[test]
fn pinned_as_measured() {
    // bash `(1 2)y`: the region ends at `)`, so bash's word is a scalar
    // assignment; here the trailing `y` gives the statement back to the ordinary
    // walk, and the tokenizer's `2` is a command word.
    let (out, err, code) = run(r#"a=(1 2)y; echo "[${a[@]}]""#);
    assert_eq!(out, "[]\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("not found"), "stderr={err:?}");
    // bash `hi` (a scoped prefix); here: the same `)`-as-command shape.
    let (out, err, code) = run(r#"a=(1 2) echo hi"#);
    assert_eq!(out, "");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("not found"), "stderr={err:?}");
    // bash 2: a newline inside the literal is an element separator; the statement
    // splitter cuts here, so the region is unclosed and this task reports it.
    let (out, err, code) = run("a=(\"x\"\n\"y\"); echo \"${#a[@]}\"");
    assert_eq!(out, "");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(
        err.contains("array assignment is missing its closing ')'"),
        "stderr={err:?}"
    );
    // bash rc 2 syntax errors; the region is bounded by the statement here.
    for script in [r#"a=(1"#, r#"a=(1 2 ; echo x)"#] {
        let (out, err, code) = run(script);
        assert_eq!(out, "");
        assert_eq!(code, 1, "{script:?} stderr={err:?}");
        assert!(
            err.contains("array assignment is missing its closing ')'"),
            "{script:?} stderr={err:?}"
        );
    }
    // bash `x2` / `[2]` / `1 2` / `[0 1]`: negative indices, slices and
    // `${!name[@]}` belong to the follow-up task; the reads this task implements
    // hand them to the ordinary arms, which answer their own way.
    check(r#"a=(1 2); echo "x${a[-1]}""#, "x\n");
    check(r#"a=(1 2); echo "[${a[@]:1:1}]""#, "[]\n");
    check(r#"a=(1 2); echo "${a[@]:-x}""#, "x\n");
    let (out, err, code) = run(r#"a=(1 2); echo "[${!a[@]}]""#);
    assert_eq!(out, "");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(
        err.contains("a[@]: invalid indirect expansion"),
        "stderr={err:?}"
    );
    // bash `2` and `declare -a a=([0]="1" [1]="2")`: `declare` is no builtin
    // here, so the statement is an external command; the store is untouched and
    // the array read answers on its own.
    let (out, err, code) = run(r#"a=(1 2); declare -a a; echo "${#a[@]}""#);
    assert_eq!(out, "2\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("not found"), "stderr={err:?}");
    let (out, err, code) = run(r#"a=(1 2); declare -p a"#);
    assert_eq!(out, "");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("not found"), "stderr={err:?}");
    // bash `hellox` (scalar `+=` is a pre-existing gap, README "Variables").
    check(r#"a=hello; a+=x; echo "$a""#, "hello\n");
    // bash `[]`: `local` scopes the string, not the store; measured rc 1 for the
    // no-function use, which the store does not change.
    check(r#"a=(1 2); f() { local a; echo "[$a]"; }; f"#, "[1]\n");
    // The `#` arm's nounset reading: bash bails for a mere scalar and answers 0
    // for a real array; fdshell's array arm answers the length through the store,
    // and an unresolved element keeps the existing nounset/zero readings.
    let (out, err, code) = run(r#"set -u; a=(1); echo "${#a[5]}""#);
    assert_eq!(out, "");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("a[5]: unbound variable"), "stderr={err:?}");
    check(r#"set -u; echo "${#q[@]}""#, "0\n");
    check(r#"set -u; a=hello; echo "${#a[0]}""#, "5\n");
}
