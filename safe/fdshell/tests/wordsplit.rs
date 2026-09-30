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
    let (out, err, code) = run(r#"x="a b"; printf %s\n $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn word_splitting_collapses_whitespace_runs() {
    let (out, err, code) = run(r#"x="  a  b "; printf %s\n $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn custom_ifs_delimits_fields() {
    let (out, err, code) = run(r"IFS=:; x=a:b; printf %s\n $x");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn custom_ifs_keeps_empty_fields() {
    let (out, err, code) = run(r#"IFS=:; x="a::b"; printf %s\n $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\n\nb\n");
}

#[test]
fn empty_ifs_disables_word_splitting() {
    let (out, err, code) = run(r#"x="a b"; IFS=; printf %s\n $x"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\n");
}

#[test]
fn quoted_expansion_does_not_split() {
    let (out, err, code) = run(r#"x="a b"; printf %s\n "$x""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\n");
}

#[test]
fn unquoted_dollar_at_splits_positional_args() {
    let (out, err, code) = run(r"set -- a b; printf %s\n $@; set --");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn set_dash_dash_splits_expansion() {
    let (out, err, code) = run(r#"x="a b"; set -- $x; printf "%s|%s" $0 $1"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a|b");
}

#[test]
fn read_ifs_updates_word_splitting() {
    let (out, err, code) = run_stdin(r"read IFS; x=a:b; printf %s\n $x", ":\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn export_ifs_updates_word_splitting() {
    let (out, err, code) = run(r"export IFS=:; x=a:b; printf %s\n $x");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn param_op_ifs_assign_updates_word_splitting() {
    let (out, err, code) = run(r"IFS=; y=a,b; x=${IFS:=,}; printf %s\n $y");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn for_ifs_updates_word_splitting() {
    let (out, err, code) = run(r"for IFS in ,; do x=a,b; printf %s\n $x; done");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn unquoted_dollar_at_custom_ifs_splits_per_positional() {
    let (out, err, code) = run(r"IFS=:; set -- a:b c; printf %s\n $@");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\nc\n");
}

#[test]
fn unquoted_dollar_at_empty_ifs_keeps_positionals() {
    let (out, err, code) = run(r#"set -- "a b" c; IFS=; printf %s\n $@"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\nc\n");
}

#[test]
fn quoted_dollar_star_custom_ifs_joins_with_first_ifs_byte() {
    let (out, err, code) = run(r#"IFS=:; set -- a b; printf %s\n "$*""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a:b\n");
}

#[test]
fn quoted_dollar_star_empty_ifs_joins_with_nothing() {
    let (out, err, code) = run(r#"IFS=; set -- a b; printf %s\n "$*""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "ab\n");
}

#[test]
fn embedded_dollar_at_uses_first_ifs_byte_join() {
    let (out, err, code) = run(r"IFS=:; set -- a b; printf %s\n x$@");
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
    // The function-call path (function_call.rs). Uses `$1` (not `$@`): a
    // function's `$1` is correctly 1-based, while `$@` currently also yields
    // the function name (pre-existing positional-model bug, task #134).
    let (out, err, code) = run(r#"f() { printf "[%s]\n" "$1"; }; f ""$(true)"#);
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
    // With `IFS=`, the word splits to one field `  ` and must stay that. Uses
    // `"$@"` (not `"$1"`): `$1` after `set --` is off-by-one today
    // (pre-existing positional-model bug, task #134).
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
