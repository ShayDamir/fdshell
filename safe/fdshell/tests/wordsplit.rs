#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");
const ARG_PRINT: &str = env!("ARG_PRINT_PATH");

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

fn run_stdin(script: &str, stdin: &str) -> (String, String, i32) {
    use std::io::Write;
    let mut child = Command::new(BIN)
        .args(["-c", script])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.stdin.take().unwrap();
    let output = child.wait_with_output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn unquoted_expansion_splits_on_default_ifs() {
    // The format is quoted so `printf` interprets `\n` itself: POSIX #4.1
    // removes the backslash of an unquoted `\n`, so `printf %s\n` prints `n`.
    let (out, err, code) = run(r#"x="a b"; printf "%s\n" $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn word_splitting_collapses_whitespace_runs() {
    let (out, err, code) = run(r#"x="  a  b "; printf "%s\n" $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn custom_ifs_delimits_fields() {
    let (out, err, code) = run(r#"IFS=:; x=a:b; printf "%s\n" $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn custom_ifs_keeps_empty_fields() {
    let (out, err, code) = run(r#"IFS=:; x="a::b"; printf "%s\n" $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\n\nb\n");
}

#[test]
fn empty_ifs_disables_word_splitting() {
    let (out, err, code) = run(r#"x="a b"; IFS=; printf "%s\n" $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\n");
}

#[test]
fn quoted_expansion_does_not_split() {
    let (out, err, code) = run(r#"x="a b"; printf "%s\n" "$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\n");
}

#[test]
fn unquoted_dollar_at_splits_positional_args() {
    let (out, err, code) = run(r#"set -- a b; printf "%s\n" $@; set --"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn set_dash_dash_splits_expansion() {
    let (out, err, code) = run(r#"x="a b"; set -- $x; printf "%s|%s" $1 $2"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a|b");
}

#[test]
fn read_ifs_updates_word_splitting() {
    let (out, err, code) = run_stdin(r#"read IFS; x=a:b; printf "%s\n" $x"#, ":\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn export_ifs_updates_word_splitting() {
    let (out, err, code) = run(r#"export IFS=:; x=a:b; printf "%s\n" $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn param_op_ifs_assign_updates_word_splitting() {
    let (out, err, code) = run(r#"IFS=; y=a,b; x=${IFS:=,}; printf "%s\n" $y"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn for_ifs_updates_word_splitting() {
    let (out, err, code) = run(r#"for IFS in ,; do x=a,b; printf "%s\n" $x; done"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn unquoted_dollar_at_custom_ifs_splits_per_positional() {
    let (out, err, code) = run(r#"IFS=:; set -- a:b c; printf "%s\n" $@"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\nc\n");
}

#[test]
fn unquoted_dollar_at_empty_ifs_keeps_positionals() {
    let (out, err, code) = run(r#"set -- "a b" c; IFS=; printf "%s\n" $@"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\nc\n");
}

#[test]
fn quoted_dollar_star_custom_ifs_joins_with_first_ifs_byte() {
    let (out, err, code) = run(r#"IFS=:; set -- a b; printf "%s\n" "$*""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a:b\n");
}

#[test]
fn quoted_dollar_star_empty_ifs_joins_with_nothing() {
    let (out, err, code) = run(r#"IFS=; set -- a b; printf "%s\n" "$*""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "ab\n");
}

#[test]
fn embedded_dollar_at_uses_first_ifs_byte_join() {
    let (out, err, code) = run(r#"IFS=:; set -- a b; printf "%s\n" x$@"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "xa\nb\n");
}

#[test]
fn mixed_quoted_token_stays_one_word() {
    // regression: quoted IFS inside a mixed token used to split the
    // word (one argv entry silently became two).
    let (out, err, code) = run(r#"builtin printf "[%s]" x"a b"c"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[xa bc]");
}

#[test]
fn mixed_quoted_expansion_stays_one_word() {
    let (out, err, code) = run(r#"x="a b"; builtin printf "[%s]" y"$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[ya b]");
}

#[test]
fn mixed_unquoted_expansion_still_splits() {
    let (out, err, code) = run(r#"x="a b"; builtin printf "[%s]" y$x z"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[ya][b][z]");
}

#[test]
fn mixed_quoted_non_whitespace_ifs_protected() {
    let (out, err, code) = run(r#"IFS=:; x=a:b; builtin printf "[%s]" y"$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[ya:b]");
}

#[test]
fn quoted_ifs_after_cmd_subst_stays_one_word() {
    // The quoted `:` after the substitution must keep its protection even
    // though the `$( )` span consumed several input bytes.
    let (out, err, code) = run(r#"IFS=:; builtin printf "[%s]" x$(true)":z""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[x:z]");
}

#[test]
fn unquoted_ifs_after_cmd_subst_still_splits() {
    let (out, err, code) = run(r#"IFS=:; builtin printf "[%s]" x$(true):y"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[x][y]");
}

#[test]
fn quoted_middle_of_word_keeps_ifs_and_splits_around_it() {
    // Unquoted IFS outside the quotes still delimits; quoted IFS stays.
    let (out, err, code) = run(r#"x="a b"; builtin printf "[%s]" "$x" c"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a b][c]");
}

#[test]
fn empty_quoted_word_counts_as_positional() {
    let (out, _err, code) = run(r#"set -- a "" b; echo $#"#);
    assert_eq!(code, 0);
    assert_eq!(out, "3\n");
}

#[test]
fn quoted_dollar_at_preserves_empty_positional() {
    let (out, _err, code) = run(r#"set -- a "" b; printf "[%s]\n" "$@""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a]\n[]\n[b]\n");
}

#[test]
fn unquoted_dollar_at_drops_empty_positional() {
    // Unquoted $@ word-splits each positional; empty ones vanish (bash).
    let (out, _err, code) = run(r#"set -- a "" b; printf "[%s] " $@; echo"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a] [b] \n");
}

#[test]
fn unquoted_dollar_at_reassignment_drops_empty_positional() {
    // Guard: `set -- $@` re-splits the positionals, so the empty one is lost.
    let (out, _err, code) = run(r#"set -- a "" b; set -- $@; echo $#"#);
    assert_eq!(code, 0);
    assert_eq!(out, "2\n");
}

#[test]
fn quoted_dollar_star_keeps_empty_element() {
    // `a` + sep + `` + sep + `b` → two spaces between `a` and `b`.
    let (out, _err, code) = run(r#"set -- a "" b; printf "[%s] " "$*"; echo"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a  b] \n");
}

#[test]
fn empty_quoted_arg_stays_one_empty_word() {
    let (out, _err, code) = run(r#"printf "[%s]\n" "" x"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[]\n[x]\n");
}

#[test]
fn empty_quoted_arg_builtin_form_stays_one_empty_word() {
    let (out, _err, code) = run(r#"builtin printf "[%s]\n" "" x"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[]\n[x]\n");
}

/// `exec`/`become` drop the binary from the word vector — the quote mask
/// vector must be sliced by the same offset, or every argument inherits
/// its predecessor's quoting.
#[test]
fn exec_builtin_first_keeps_quoted_arg_whole() {
    let (out, err, code) = run(r#"exec printf "[%s] " x "a b""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[x] [a b] ");
}

/// Pins the *offset*, not just the bug: the format word is quoted and the
/// argument is not, so an off-by-two mask slice word-splits `"a b"`.
#[test]
fn exec_builtin_first_masks_stay_aligned_after_binary() {
    let (out, err, code) = run(r#"exec printf "[%s] " "a b" c"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a b] [c] ");
}

/// The external branch: the resolved binary is not a builtin, so this goes
/// through `resolve_path` + `execveat`.
#[test]
fn become_external_keeps_quoted_arg_whole() {
    let (out, err, code) = run(&format!("become {ARG_PRINT} \"a b\" c"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a b] [c] \n");
}

#[test]
fn for_list_keeps_empty_quoted_word() {
    let (out, _err, code) = run(r#"for w in "" x; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[]\n[x]\n");
}

#[test]
fn shift_past_empty_first_positional() {
    let (out, _err, code) = run(r#"set -- "" a b; shift; printf "[%s]\n" "$@""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a]\n[b]\n");
}

#[test]
fn empty_quoted_command_fails_like_unknown_command() {
    // `"" cmd` no longer silently runs `cmd`; the empty command word is
    // unresolvable (bash: `: command not found`, exit 127).
    let (out, err, code) = run(r#""" true"#);
    assert_ne!(code, 0);
    assert!(err.contains("not found"), "stderr={err:?}");
    assert!(out.is_empty());
}

// Quoted-empty words with a runtime-empty expansion (task #76): a word whose
// raw text contained quotes but whose IFS split produced zero fields must be
// one empty word, as in bash. One test per word path so a failure localises
// the slice.

#[test]
fn quoted_empty_cmd_subst_is_one_positional() {
    // `set -- ""$(true)`: the `set --` path (intercept/set_cmd.rs slice).
    let (out, err, code) = run(r#"set -- ""$(true); echo $#; printf "[%s]\n" "$1""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n[]\n");
}

#[test]
fn quoted_empty_var_is_one_positional() {
    let (out, err, code) = run(r#"zz=; set -- ""$zz; echo $#; printf "[%s]\n" "$1""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n[]\n");
}

#[test]
fn quoted_empty_cmd_subst_builtin_form() {
    // The plain `printf` command: `external::run` builtin-first branch.
    let (out, err, code) = run(r#"printf "[%s]\n" a ""$(true) b"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a]\n[]\n[b]\n");
}

#[test]
fn quoted_empty_cmd_subst_builtin_keyword_form() {
    // The `builtin` keyword form: the offset-2 slice in replacer.rs.
    let (out, err, code) = run(r#"builtin printf "[%s]\n" ""$(true)"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n");
}

#[test]
fn quoted_empty_cmd_subst_function_call() {
    // The function-call path (function_call.rs). The probe is `"$@"`: a call
    // rebinds `$1..` only, so `$@` is honest inside the body.
    let (out, err, code) = run(r#"f() { printf "[%s]\n" "$@"; }; f ""$(true)"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n");
}

#[test]
fn quoted_empty_cmd_subst_exec_path() {
    // The `exec`/`become` path (intercept/become_cmd.rs → replacer::execute).
    let (out, err, code) = run(r#"exec printf "[%s]\n" ""$(true)"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n");
}

#[test]
fn quoted_empty_cmd_subst_external_path() {
    // The external branch (resolve_path + execveat) in replacer/external.rs.
    let (out, err, code) = run(&format!("become {ARG_PRINT} \"\"$(true) c"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[] [c] \n");
}

#[test]
fn quoted_empty_cmd_subst_timeout_path() {
    // The `timeout` rebuilt-`CommandLine` path.
    let (out, err, code) = run(r#"timeout 1 printf "[%s]\n" ""$(true)"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n");
}

#[test]
fn quoted_empty_cmd_subst_pipeline_stage() {
    // The flag must survive the pipeline child (pipeline/child.rs).
    let (out, err, code) = run(r#"printf "[%s]\n" ""$(true) | cat"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n");
}

#[test]
fn for_list_quoted_empty_cmd_subst_is_one_empty_iteration() {
    // The for-list path (expand.rs): one empty iteration, not zero.
    let (out, err, code) = run(r#"for w in "$(true)"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n");
}

#[test]
fn quoted_dollar_at_reassignment_no_positionals() {
    // `""$@` with no positionals is one empty word (bash).
    let (out, err, code) = run(r#"set --; set -- ""$@; echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n");
}

#[test]
fn quoted_dollar_at_reassignment_empty_positional() {
    let (out, err, code) = run(r#"set -- ""; set -- ""$@; echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n");
}

#[test]
fn quoted_dollar_star_reassignment_no_positionals() {
    let (out, err, code) = run(r#"set --; set -- ""$*; echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n");
}

#[test]
fn quoted_dollar_at_reassignment_keeps_words() {
    // Guard: words from the positionals are kept, no empty word is added.
    let (out, err, code) = run(r#"set -- a "" b; set -- ""$@; echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "2\n");
}

// Task #219: a **fully quoted whole-word** `$@` is bash's positional word
// list, not a word that expanded to nothing, so with no positionals it is no
// word at all. The discriminator is the quote mask (`"$@"` is all-quoted,
// `""$@` is all-unquoted), so the triple `"$@"` → 0, `"$*"` → 1, `""$@` → 1
// is pinned below. Probe rule: `echo $#`, a `for` loop, or `arg_print`'s argv
// count — **never** `printf "[%s]\n" "$@"`, which prints `[]` in bash too
// because printf reuses its format with empty substitutions.

#[test]
fn fully_quoted_dollar_at_no_positionals_is_no_positionals() {
    let (out, err, code) = run(r#"set --; set -- "$@"; echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "0\n");
    let (out, err, code) = run(r#"set --; set -- "$@" "$@"; echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "0\n");
    // The glued spelling `"$@"x` is one word `x` (bash), so the pair has one.
    let (out, err, code) = run(r#"set --; set -- "$@"x "$@"; echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n");
}

#[test]
fn fully_quoted_dollar_at_mixed_with_dollar_star_keeps_the_join_word() {
    // `"$*"` contributes its single (empty) joined word; `"$@"` contributes none.
    let (out, err, code) = run(r#"set --; set -- "$@" "$*"; echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n");
}

#[test]
fn fully_quoted_dollar_at_under_nounset_is_no_positionals() {
    // bash does not bail on `"$@"` with an empty deque, nounset included.
    let (out, err, code) = run(r#"set --; set -u; set -- "$@"; echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "0\n");
    assert_eq!(err, "");
}

#[test]
fn fully_quoted_dollar_at_empty_positional_is_one_positional() {
    // An empty positional *is* a word for `"$@"`: the fix must not drop it.
    let (out, err, code) = run(r#"set -- ""; set -- "$@"; echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n");
}

#[test]
fn fully_quoted_dollar_at_pipeline_stage_is_no_argument() {
    // The argv count is the observable: `arg_print` prints `[arg] ` per argv
    // entry, so bash prints a bare newline and fdshell must too.
    let (out, err, code) = run(&format!("set --; {ARG_PRINT} \"$@\" | cat"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "\n");
    // Guard: the partially quoted spelling is one empty argument.
    let (out, err, code) = run(&format!("set --; {ARG_PRINT} \"\"$@ | cat"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[] \n");
    let (out, err, code) = run(r#"set --; set -- "$@"; echo $# | cat"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "0\n");
}

#[test]
fn fully_quoted_dollar_at_builtin_gets_exactly_the_positionals() {
    // The `set --` builtin is its own word-count probe: over one positional
    // `a`, both `"$@"` and the partially quoted `""$@` hand exactly one word
    // to the builtin (bash: `1/a` in both).
    let (out, err, code) = run(r#"set -- a; set -- "$@"; echo "$#/$1""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1/a\n");
    let (out, err, code) = run(r#"set -- a; set -- ""$@; echo "$#/$1""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1/a\n");
}

#[test]
fn fully_quoted_dollar_at_external_and_timeout_get_no_argument() {
    // The `become`/`exec` slice and the `timeout` rebuilt command line see the
    // same word count: no argv entries, so `arg_print` prints a bare newline
    // (bash `timeout 1 arg_print "$@"` → newline, `""$@` → `[] `).
    let (out, err, code) = run(&format!("set --; become {ARG_PRINT} \"$@\""));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "\n");
    let (out, err, code) = run(&format!("set --; timeout 1 {ARG_PRINT} \"$@\""));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "\n");
    // Guard pair: the partially quoted spelling is one empty argument.
    let (out, err, code) = run(&format!("set --; timeout 1 {ARG_PRINT} \"\"$@"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[] \n");
}

// Regression guards: the unquoted spellings must not gain a word.

#[test]
fn unquoted_empty_cmd_subst_is_no_positionals() {
    let (out, err, code) = run(r#"set -- $(true); echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "0\n");
}

#[test]
fn unquoted_empty_cmd_subst_for_list_is_no_iterations() {
    let (out, err, code) = run(r#"for w in $(true); do echo hit; done"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
}

#[test]
fn quoted_whitespace_only_expansion_empty_ifs_keeps_spaces() {
    // With `IFS=`, the word splits to one field `  ` and must stay that; the
    // value is read with `"$@"`.
    let (out, err, code) = run(r#"IFS=; set -- ""$(echo "  "); printf "[%s]\n" "$@""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[  ]\n");
}

#[test]
fn quoted_multiword_expansion_splits_positionals() {
    let (out, err, code) = run(r#"set -- ""$(echo "a b"); echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "2\n");
}

// Task #130: a quoted region's bytes are marked quoted, so its multi-word
// output stays ONE word. The values are read with `builtin printf "[%s]"`
// (the literal-IFS byte after an embedded expansion cuts the word in pieces is
// task #222).
#[test]
fn quoted_region_expansion_stays_one_word() {
    let cases: &[(&str, &str)] = &[
        (r#"builtin printf "[%s]" "$(printf "a b")""#, "[a b]"),
        (r#"builtin printf "[%s]" a"$(echo "x y")""#, "[ax y]"),
        (r#"builtin printf "[%s]" "a$(printf " b")c""#, "[a bc]"),
        (r#"builtin printf "[%s]" "${u:-"z w"}""#, "[z w]"),
        (
            r#"builtin printf "[%s]" "$(echo "$(printf "p q")")""#,
            "[p q]",
        ),
        (
            r#"builtin printf "[%s]" "a$(printf " b")c" second"#,
            "[a bc][second]",
        ),
        // The region's own inner quotes are quoted data: `xa bz` is one word.
        (
            r#"IFS=:; builtin printf "[%s]" "x$(printf "a b")"z"#,
            "[xa bz]",
        ),
        // Guard (the quote closes *before* `$(`): the region is unquoted, so
        // the word splits on IFS exactly as before task #130.
        (r#"IFS=:; builtin printf "[%s]" x$(true)":z""#, "[x:z]"),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(err, "", "{script:?} must report nothing, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}

// Task #133: field splitting applies to the result of an expansion only
// (POSIX 2.6.5), so a literal word — no `$`, no `%`, no leading `~` — is one
// field whatever `IFS` is. Measured against `bash -c` 5.3.9.

#[test]
fn literal_word_with_non_whitespace_ifs_stays_one_arg() {
    let (out, err, code) = run(r#"IFS=,; builtin printf "[%s]\n" a,b"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a,b]\n");
}

#[test]
fn literal_word_with_ifs_byte_is_one_positional() {
    let (out, err, code) = run(r#"IFS=:; set -- a:b; echo $#"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n");
}

#[test]
fn literal_leading_ifs_byte_is_no_leading_empty_positional() {
    let (out, err, code) = run(r#"IFS=,; set -- ,a; printf "[%s]" "$@""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[,a]");
}

#[test]
fn literal_leading_tilde_user_word_with_ifs_byte_stays_one_arg() {
    // `~user` is literal here (README), so the word performed no expansion:
    // bash splits nothing. fdshell keeps `~nobody` literal, so its field is
    // `~nobody:x` where bash's is `/var/empty:x` (the same field count).
    let (out, err, code) = run(r#"IFS=:; builtin printf "[%s]" ~nobody:x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[~nobody:x]");
}

// Task #133, defect 2: a non-whitespace `IFS` delimiter at the very end of an
// expansion produces no trailing empty field; leading and middle empties stay.
// Measured against `bash -c` 5.3.9.

#[test]
fn cmd_subst_trailing_ifs_delimiter_is_no_trailing_positional() {
    let (out, err, code) = run(r#"IFS=,; set -- $(echo "a,"); printf "[%s]" "$@""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a]");
    let (out, _err, code) = run(r#"IFS=,; set -- $(printf ","); echo $#"#);
    assert_eq!(code, 0);
    assert_eq!(out, "1\n");
}

#[test]
fn cmd_subst_all_delimiters_keeps_leading_empties() {
    // `,,,`: two leading empty fields, the trailing one elided.
    let (out, err, code) = run(r#"IFS=,; set -- $(echo ",,,"); printf "[%s]" "$@""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[][][]");
}

#[test]
fn positional_re_split_elides_trailing_empty_field() {
    // The unquoted `$@` re-split path (`positional.rs`): each positional is
    // split on `IFS`, so its trailing delimiter loses its empty field.
    let (out, err, code) = run(r#"IFS=,; set -- "a,"; printf "[%s]" $@"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a]");
}

#[test]
fn whitespace_delimiter_run_after_ifs_delimiter_trims_it() {
    // `IFS=" ,"`: the delimiter merges with the trailing whitespace run and
    // both are trimmed — one field `a`, not `a, ` nor `a, []`.
    let (out, err, code) = run(r#"IFS=" ,"; builtin printf "[%s]" $(printf "a, ")"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a]");
}

#[test]
fn leading_and_middle_empty_fields_stay() {
    // Regression guard for the elision rule: only the very end of the
    // expansion is affected.
    let (out, err, code) = run(r#"IFS=,; set -- $(echo ",a"); printf "[%s]\n" "$@""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n[a]\n");
    let (out, _err, code) = run(r#"IFS=,; set -- $(echo "a,,"); printf "[%s]\n" "$@""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a]\n[]\n");
}

// Task #222 (accepted divergence, next to task #133's fix): the mask model
// splits the whole concatenation of a literal part and an expansion, while
// bash splits only the expansion part — a literal `IFS` byte placed after an
// unquoted embedded expansion cuts between the expansion's last field and the
// literal text.

#[test]
fn literal_ifs_byte_after_expansion_cuts_the_word() {
    let cases: &[(&str, &str)] = &[
        (
            r#"IFS=,; v=$(echo "b c"); builtin printf "[%s]" x$v,y"#,
            "[xb c][y]",
        ),
        (
            r#"IFS=,; set -- $(echo "b c"); builtin printf "[%s]" x$@,y"#,
            "[xb c][y]",
        ),
        (
            r#"IFS=,; set -- "$(echo "b c")"; builtin printf "[%s]" x"$@"z,y"#,
            "[xb cz][y]",
        ),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}

// Task #223: a quoted expansion glued to literal text must keep every byte.
// These cases run the word through the **split path** (word text out of the
// substitution pass, into the splitter); the assignment path, where the same
// word becomes a variable value, is pinned beside them in
// `tests/env_assign.rs::quoted_expansion_glued_to_literal_text_in_an_assignment_value`.
// The assignment form is split-independent, so these pins state the bytes
// only: what the splitter decides about a literal delimiter is task #222's
// rule. Before the fix these words lost the expansion's output and the byte
// right after it (`"$v"b,c` → `[] [c]`, `"$v"bc` → the value of `vb`).

#[test]
fn quoted_expansion_glued_to_literal_text_keeps_every_byte() {
    // bash: `IFS=,; v=q; w="$v"b,c; printf "[%s]" "$w"` → `[qb,c]`.
    let cases: &[(&str, &str)] = &[
        (
            r#"IFS=,; v=q; w="$v"b,c; builtin printf "[%s]" "$w""#,
            "[qb,c]",
        ),
        (
            r#"IFS=,; v=""; w="$v"a,b; builtin printf "[%s]" "$w""#,
            "[a,b]",
        ),
        (
            r#"IFS=,; v=q; w=x"$v"b,c; builtin printf "[%s]" "$w""#,
            "[xqb,c]",
        ),
        // A glued literal that is a name prefix must not extend the name:
        // `vb` is bound and the word must still print `[qb]`, never `[VB]`.
        (r#"v=q; vb=VB; builtin printf "[%s]" "$v"b"#, "[qb]"),
        (r#"v=q; vb=VB; builtin printf "[%s]" ab"$v"cd"#, "[abqcd]"),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}

#[test]
fn quoted_expansion_glued_to_literal_text_is_one_word() {
    // bash `IFS=,; v=q; vbc=VBC; set -- "$v"bc` → the single field `[qbc]`:
    // no byte loss, no word split, and the name stays `v`.
    let (out, err, code) = run(r#"IFS=,; v=q; vbc=VBC; set -- "$v"bc; printf "[%s]\n" "$@""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[qbc]\n");
    let (out, err, code) = run(r#"IFS=,; v=q; vbc=VBC; set -- "$v"bc; echo "$#/$1""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1/qbc\n");
}

#[test]
fn quoted_expansion_glued_to_a_literal_delimiter_keeps_all_the_bytes() {
    // The `IFS=,` `set --` shapes of task #223, pinned to today's *bytes*:
    // every byte survives, and the word still cuts at the literal `,`. The
    // join bash performs here (`[qb,c]` as one field) is task #222's
    // accepted divergence, so #222's own commit is the one that moves these
    // expectations — this task must not pre-empt it.
    let cases: &[(&str, &str)] = &[
        (
            r#"IFS=,; v=q; vb=VB; set -- "$v"b,c; printf "[%s]\n" "$@""#,
            "[qb]\n[c]\n",
        ),
        (
            r#"IFS=,; v=""; va=VA; set -- "$v"a,b; printf "[%s]\n" "$@""#,
            "[a]\n[b]\n",
        ),
        (
            r#"IFS=,; v=q; vb=VB; set -- x"$v"b,c; printf "[%s]\n" "$@""#,
            "[xqb]\n[c]\n",
        ),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}
