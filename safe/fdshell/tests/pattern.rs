#![allow(clippy::unwrap_used)]

use std::process::{Command, Stdio};
use std::str;
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// A per-test scratch directory (tests in one binary share the pid, so the
/// counter keeps the directories from colliding).
fn scratch() -> std::path::PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-pattern-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Every expression is assigned to a variable, then all variables are printed
/// with `printf "[%s]"`. The assignment word is unquoted, so the pattern bytes
/// stay live (a fully quoted word makes them literal — pinned by
/// `enclosing_quotes_make_pattern_bytes_literal`), and `printf` keeps the empty
/// results visible (`echo` drops an empty unquoted word, as bash does).
/// All pinned values are the bash 5.3.9 outputs measured on this machine.
fn strips(defs: &str, exprs: &[&str]) -> String {
    let mut script = String::from(defs);
    let mut prints = Vec::new();
    for (i, e) in exprs.iter().enumerate() {
        script.push_str(&format!("; r{i}={e}"));
        prints.push(format!("\"$r{i}\""));
    }
    let (out, err, code) = run(&format!("{script}; printf \"[%s]\" {}", prints.join(" ")));
    assert_eq!(code, 0, "stderr={err}");
    out
}

// --- the anchored shortest/longest prefix and suffix ---

#[test]
fn prefix_and_suffix_strips_match_bash() {
    // bash 5.3.9 pins, measured on this machine: `#`/`##` anchor at the start,
    // `%`/`%%` at the end, shortest/longest.
    assert_eq!(
        strips(
            "v=abcabc",
            &[
                "${v#a}",
                "${v##a}",
                "${v#a*c}",
                "${v##a*c}",
                "${v%?}",
                "${v%%abc}",
                "${v#c*}",
                "${v#z}",
            ]
        ),
        "[bcabc][bcabc][abc][][abcab][abc][abcabc][abcabc]"
    );
}

#[test]
fn star_pattern_shortest_and_longest_in_both_directions() {
    assert_eq!(
        strips("s=aXbXc", &["${s#*X}", "${s##*X}", "${s%X*}", "${s%%X*}"]),
        "[bXc][c][aXb][a]"
    );
}

#[test]
fn longest_star_empties_the_whole_value() {
    // The bounds of `strip_len`: a longest candidate of the full length, and the
    // `n - k` suffix slice, both give the empty result.
    assert_eq!(
        strips(
            "v=aaaa",
            &[
                "${v##a*}", "${v%%a*}", "${v#a*}", "${v%a*}", "${v##*a}", "${v%%*a}",
            ]
        ),
        "[][][aaa][aaa][][]"
    );
    assert_eq!(strips("v=abca", &["${v##a*}", "${v%%*a}"]), "[][]");
}

#[test]
fn prefix_and_suffix_on_a_value_ending_with_the_pattern() {
    // The `#`/`%` direction pin: `v=abca` starts and ends with `a`.
    assert_eq!(
        strips("v=abca", &["${v#a}", "${v%a}", "${v##a}", "${v%%a}"]),
        "[bca][abc][bca][abc]"
    );
}

#[test]
fn path_strips() {
    assert_eq!(
        strips(
            "p=/a/b/c.txt",
            &[
                "${p##*/}", "${p%.*}", "${p%/*}", "${p%%/*}", "${p#*/}", "${p#*/*}",
            ]
        ),
        "[c.txt][/a/b/c][/a/b][][a/b/c.txt][a/b/c.txt]"
    );
}

#[test]
fn dot_is_ordinary_in_a_word_pattern() {
    // No FNM_PERIOD rule: a leading `*`/`?` may consume a leading `.`
    // (`d=.abc; ${d#*c}` strips the whole value), unlike the pathname glob.
    assert_eq!(
        strips(
            "d=.abc",
            &["${d#*c}", "${d#c*}", "${d##*}", "${d#?}", "${d#.*}"]
        ),
        "[][.abc][][abc][abc]"
    );
    assert_eq!(strips("q=a.b.c", &["${q%%.*}", "${q%.*}"]), "[a][a.b]");
}

#[test]
fn empty_pattern_and_star_strip_nothing_shortest_all_longest() {
    assert_eq!(
        strips(
            "v=abcabc",
            &[
                "${v#}", "${v##}", "${v%}", "${v%%}", "${v#*}", "${v##*}", "${v%*}", "${v%%*}",
            ]
        ),
        "[abcabc][abcabc][abcabc][abcabc][abcabc][][abcabc][]"
    );
}

#[test]
fn no_match_leaves_the_value_whole() {
    assert_eq!(
        strips(
            "v=abcabc",
            &[
                "${v#z}", "${v#a#}", "${v#a%b}", "${v#a,b}", "${v#[]}", "${v#[a}", "${v#[]a}",
            ]
        ),
        "[abcabc][abcabc][abcabc][abcabc][abcabc][abcabc][abcabc]"
    );
}

#[test]
fn bracket_ranges_negation_and_classes() {
    assert_eq!(
        strips(
            "v=abcabc",
            &[
                "${v#[ab]*}",
                "${v#[a-b]*}",
                "${v#[!b]*}",
                "${v#[a-c]bc}",
                "${v#[[:alpha:]]bc}",
                "${v#[abc]}",
            ]
        ),
        "[bcabc][bcabc][bcabc][abc][abc][bcabc]"
    );
}

#[test]
fn escaped_and_quoted_pattern_bytes_are_literal() {
    // `\X` is a literal pair and a quoted pattern byte is literal (bash, with
    // the word unquoted): the `*` matches nothing, so the value stays whole.
    let (out, err, code) = run("esc=a*c; printf \"[%s]\" ${esc#\\*} ${esc#\"*\"} ${esc#\"a\"c}");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[a*c][a*c][a*c]");
    // An escape pair that consumes the first byte strips it (bash).
    let (out, err, code) = run("esc=a*c; printf \"[%s]\" ${esc#a\\*}");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[c]");
}

#[test]
fn quoted_pattern_bytes_keep_their_mask_bit_in_the_braces() {
    // The pattern's mask is the word's mask slice, so a quoted pattern byte is
    // literal and an unquoted one is live, in any order.
    assert_eq!(
        strips(
            "v=abcabc",
            &["${v#\"a\"b}", "${v#\"ab\"}", "${v#a\"b\"}", "${v#\"a\"}",]
        ),
        "[cabc][cabc][cabc][bcabc]"
    );
    let (out, err, code) = run(
        "v=abcabc; printf \"[%s]\" ${v#\"[\"a]} ${v#\"[\"a][b]} ${v#\"a\"b} ${v#a\"b\"} ${v#\"ab\"}",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[abcabc][abcabc][cabc][cabc][cabc]");
}

#[test]
fn stripped_result_is_a_normal_word_and_globs() {
    // `a*x` stripped to `*x` is an unquoted pattern word, so it globs to the
    // scratch file `zx` — and a fully quoted word stays literal.
    let dir = scratch();
    std::fs::write(dir.join("zx"), "").unwrap();
    let dir = dir.to_str().unwrap().to_string();
    let (out, err, code) = run(&format!("cd {dir}; v=a*x; echo ${{v#a}}"));
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "zx\n");
    let (out, err, code) = run(&format!("cd {dir}; v=a*x; echo \"${{v#a}}\""));
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "*x\n");
    std::fs::remove_dir_all(&dir).unwrap();
}

// --- the operator scan is positional ---

#[test]
fn colon_and_pattern_operators_keep_their_positional_precedence() {
    // The first operator byte wins: `${v#a:}` is the pattern `a:` (no match),
    // `${v:-x#y}` stops at the colon so `#` stays inside the colon word, and a
    // name keeps its own `:` (`${var:x:-w}` reads the name `var:x`).
    let (out, err, code) = run("v=abcabc; printf \"[%s]\" ${v#a:} ${v:-x#y} ${v#a,b}");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[abcabc][abcabc][abcabc]");
    let (out, err, code) = run("var:x=v; printf \"[%s]\" \"${var:x:-w}\"");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[v]");
}

#[test]
fn single_char_operators_are_not_pattern_operators() {
    // Accepted divergence: fdshell implements only the colon-prefixed operators,
    // so `${v+a#y}` reads the name `v+a` (unset → empty) where bash has the `+`
    // operator with the word `x#y` (`${v+x#y}` → `x#y`).
    assert_eq!(
        strips("v=abcabc", &["${v+a#y}", "${v=a#b}", "${v:v}"]),
        "[][][]"
    );
}

#[test]
fn pattern_may_contain_the_operator_bytes() {
    assert_eq!(
        strips("v=abcabc", &["${v#a#}", "${v#a%b}", "${v#}x}", "${v#a}x}"]),
        "[abcabc][abcabc][abcabcx}][bcabcx}]"
    );
    // The first `}` closes the brace; the tail is literal.
    let (out, err, code) = run("v=abcabc; echo \"${v#}x}\" \"${v#a}x}\"");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "abcabcx} bcabcx}\n");
}

// --- names: indirect, unset, assignment, nounset ---

#[test]
fn indirect_name_with_a_pattern() {
    assert_eq!(
        strips("ind=v; v=abcabc", &["${!ind#a}", "${!ind%bc}", "${!ind}"]),
        "[bcabc][abca][abcabc]"
    );
}

#[test]
fn unset_parameter_expands_to_empty_with_a_pattern() {
    assert_eq!(strips("", &["${undef#q}", "${undef%%*}"]), "[][]");
    assert_eq!(strips("empty=", &["${empty#*}"]), "[]");
}

#[test]
fn nounset_bails_an_unbound_pattern_parameter() {
    // The pattern family has no word to fall back on, so `set -u` bails it.
    for script in [
        "set -u; printf \"[%s]\" \"${undef#q}\"",
        "set -u; printf \"[%s]\" \"${undef#*}\"",
    ] {
        let (_out, err, code) = run(script);
        assert_eq!(code, 1, "stderr={err}");
        assert!(err.contains("undef: unbound variable"), "stderr={err}");
    }
    let (out, err, code) = run("set -u; v=abcabc; printf \"[%s]\" \"${v#q}\"");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[abcabc]");
}

#[test]
fn nounset_bails_the_indirect_with_a_pattern() {
    let (_out, err, code) = run("set -u; printf \"[%s]\" \"${!nope#a}\"");
    assert_eq!(code, 1, "stderr={err}");
    assert!(
        err.contains("nope: invalid indirect expansion"),
        "stderr={err}"
    );
    // `p` names the indirect name `n`, which is unbound: bash's message keeps
    // the `!`.
    let (_out, err, code) = run("set -u; p=n; printf \"[%s]\" \"${!p#a}\"");
    assert_eq!(code, 1, "stderr={err}");
    assert!(err.contains("!p: unbound variable"), "stderr={err}");
    let (_out, err, code) = run("set -u; ind=nope; printf \"[%s]\" \"${!ind#a}\"");
    assert_eq!(code, 1, "stderr={err}");
    assert!(err.contains("!ind: unbound variable"), "stderr={err}");
    let (out, err, code) = run("set -u; ind=v; v=abcabc; printf \"[%s]\" \"${!ind#a}\"");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[bcabc]");
}

#[test]
fn nounset_stays_exempt_for_the_colon_operators() {
    // The colon family supplies its own word, so `set -u` never bails it.
    let (out, err, code) =
        run("set -u; printf \"[%s]\" \"${undef:-x}\" \"${undef:+y}\" \"${undef:-x#a}\"");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[x][][x#a]");
}

#[test]
fn assignment_form_stores_the_stripped_value() {
    // `x=${v#a}` is the bare-assignment path, so `x` holds the stripped text.
    assert_eq!(
        strips("v=abcabc; x=${v#a}", &["${x#b}", "$x"]),
        "[cabc][bcabc]"
    );
}

#[test]
fn set_f_does_not_change_the_pattern_operators() {
    // `noglob` is a pathname-glob option; a word pattern is matched with no
    // filesystem, so `set -f` is a no-op here (bash measures the same).
    let (out, err, code) = run("set -f; v=abcabc; printf \"[%s]\" ${v#*a} ${v#?} ${v%%abc}");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[bcabc][bcabc][abc]");
}

#[test]
fn unclosed_brace_stays_literal() {
    let (out, err, code) = run("v=abcabc; printf \"[%s]\" \"${v#a\"");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[${v#a]");
}

#[test]
fn long_value_with_no_match_stays_whole() {
    // 10 000 chars with no match: the candidate scan is iterative, so this
    // proves a future change cannot turn it into recursion (LESSONS).
    let script = "long=$(printf a%.0s {1..10000}); printf \"%s\" ${long#zz} | cut -c1-3";
    let (out, err, code) = run(script);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "aaa\n");
    // A longest star that matches the whole value empties it (bash).
    let (out, err, code) = run("long=$(printf a%.0s {1..10000}); printf \"%s\" ${long##a*}");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "");
}

// --- accepted divergences from bash (README) ---

#[test]
fn pattern_word_is_not_expanded() {
    // bash expands `$…`/`$(…)`/`$((…))` inside `${…}`; fdshell keeps the
    // pattern bytes literal (task #174 documents this).
    assert_eq!(
        strips("v=abcabc", &["${v#$(printf abc)}", "${v#$v}"]),
        "[abcabc][abcabc]"
    );
}

#[test]
fn enclosing_quotes_make_pattern_bytes_literal() {
    // fdshell's quote rule is byte-level: a quoted pattern byte is literal, and
    // a fully quoted word masks every pattern byte. bash removes the enclosing
    // quotes before matching, so it strips; the unquoted forms match bash.
    let (out, err, code) = run("v=abcabc; printf \"[%s]\" \"${v#a*c}\" \"${v#a}\" \"${v#z}\"");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[abcabc][bcabc][abcabc]");
    // The unquoted forms match bash; the assignment form keeps the empty
    // longest result visible (an unquoted empty word is dropped, as in bash).
    assert_eq!(strips("v=abcabc", &["${v#a*c}", "${v##a*c}"]), "[abc][]");
    // A pattern with a quoted byte inside the braces is literal in both shells.
    let (out, err, code) = run("esc=a*c; printf \"[%s]\" ${esc#\"*\"}");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[a*c]");
}

#[test]
fn assignment_value_drops_the_quote_mask() {
    // The bare-assignment path expands its value with no mask (the codebase
    // rule), so a quoted `[` in the pattern is treated as unquoted here:
    // fdshell strips `a`, bash (whose `"` is literal in the pattern) does not.
    assert_eq!(strips("v=abcabc", &["${v#\"[\"a]}"]), "[bcabc]");
}

#[test]
fn unquoted_space_inside_the_braces_splits_the_word() {
    // fdshell splits words at an unquoted space, so a pattern with a space
    // needs the byte quoted; a fully quoted word keeps the pattern literal.
    // bash keeps the space inside `${…}` and strips, so `[b]` is the bash value.
    let (out, err, code) = run("sp=\"a b\"; printf \"[%s]\" \"[${sp#* }]\"");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[[a b]]");
}

#[test]
fn empty_name_with_a_pattern_operator_is_empty() {
    // bash rejects `${%x}` (`bad substitution`, rc 1); fdshell strips the empty
    // parameter, which expands to the empty string at rc 0.
    let (out, err, code) = run("printf \"[%s]\" \"${%x}\"");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[]");
}

#[test]
fn positional_parameter_in_braces_is_not_a_pattern_target() {
    // `${1#a}` is the positional parameter `1` in bash; fdshell reads `1` as a
    // variable name, which is unset, so it expands to empty (task #177).
    assert_eq!(strips("set -- a b", &["${1#a}", "${1}"]), "[][]");
}

#[test]
fn length_operator_with_a_pattern_reads_the_whole_body_as_a_name() {
    // `${#v#a}` is a `bad substitution` (rc 1) in bash; fdshell reads the name
    // `v#a` (unset) and prints `0` at rc 0 — task #173 owns this arm.
    let (out, err, code) = run("v=abcabc; printf \"[%s]\" \"${#v#a}\"");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[0]");
}
