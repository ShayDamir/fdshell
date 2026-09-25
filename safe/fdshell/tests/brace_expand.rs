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

// --- comma groups ----------------------------------------------------------

#[test]
fn comma_group_expands() {
    let (out, err, code) = run("echo {a,b,c}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b c\n");
}

#[test]
fn comma_groups_cross_product() {
    let (out, err, code) = run("echo {a,b}{c,d}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "ac ad bc bd\n");
}

#[test]
fn single_element_group_is_literal() {
    // `{a}` has no comma/sequence, so it does not expand; only `{c,d}` does.
    let (out, err, code) = run("echo {a}b{c,d}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "{a}bc {a}bd\n");
}

#[test]
fn nested_comma_group() {
    let (out, err, code) = run("echo {a,{b,c}}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b c\n");
}

#[test]
fn empty_element_drops_on_word_split() {
    let (out, err, code) = run("echo {a,,b}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\n");
}

#[test]
fn empty_group_yields_no_args() {
    // `{,}` expands to two empty words, which vanish: `echo` gets no args.
    let (out, err, code) = run("echo {,}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "\n");
}

#[test]
fn empty_word_cross_product() {
    let (out, err, code) = run("echo {a,}{,}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a a\n");
}

// --- sequences -------------------------------------------------------------

#[test]
fn ascending_sequence() {
    let (out, err, code) = run("echo {1..5}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1 2 3 4 5\n");
}

#[test]
fn descending_sequence() {
    let (out, err, code) = run("echo {10..1}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "10 9 8 7 6 5 4 3 2 1\n");
}

#[test]
fn char_sequence() {
    let (out, err, code) = run("echo {a..c}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b c\n");
}

#[test]
fn descending_char_sequence() {
    let (out, err, code) = run("echo {z..a}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "z y x w v u t s r q p o n m l k j i h g f e d c b a\n");
}

#[test]
fn sequence_with_step() {
    let (out, err, code) = run("echo {1..5..2}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1 3 5\n");
}

#[test]
fn char_sequence_with_step() {
    let (out, err, code) = run("echo {a..c..2}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a c\n");
}

#[test]
fn negative_step() {
    let (out, err, code) = run("echo {1..5..-2}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1 3 5\n");
}

#[test]
fn zero_step_is_one() {
    let (out, err, code) = run("echo {1..10..0}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1 2 3 4 5 6 7 8 9 10\n");
}

#[test]
fn zero_padded_sequence() {
    let (out, err, code) = run("echo {01..03}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "01 02 03\n");
}

#[test]
fn zero_padded_wider_than_end() {
    let (out, err, code) = run("echo {010..12}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "010 011 012\n");
}

#[test]
fn zero_padded_crossing_sign() {
    let (out, err, code) = run("echo {-01..1}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "-01 000 001\n");
}

#[test]
fn zero_padded_descending_to_negative() {
    let (out, err, code) = run("echo {01..-1}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "01 00 -1\n");
}

#[test]
fn rhs_zero_padding_drives_width() {
    // The padding width comes from the padded side, even when it is the end.
    let (out, err, code) = run("echo {1..05}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "01 02 03 04 05\n");
    let (out, err, code) = run("echo {1..-05}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "001 000 -01 -02 -03 -04 -05\n");
}

#[test]
fn sequence_cross_product() {
    let (out, err, code) = run("echo {1..2}{3..4}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "13 14 23 24\n");
}

// --- mixed / nested --------------------------------------------------------

#[test]
fn inner_comma_in_invalid_sequence() {
    // The outer sequence is invalid, but the inner comma group still expands.
    let (out, err, code) = run("echo {1..{a,b}3}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "{1..a3} {1..b3}\n");
}

#[test]
fn invalid_sequence_with_postamble() {
    let (out, err, code) = run("echo {x..1}y{d,e}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "{x..1}yd {x..1}ye\n");
}

#[test]
fn invalid_sequence_literal_times_postamble() {
    let (out, err, code) = run("echo {a..b..c}x{d,e}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "{a..b..c}xd {a..b..c}xe\n");
}

#[test]
fn comma_outranks_dots() {
    // The unquoted comma makes the whole group a comma group: `a..b` and `c`.
    let (out, err, code) = run("echo {a..b,c}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a..b c\n");
}

#[test]
fn comma_of_two_invalid_sequences() {
    let (out, err, code) = run("echo {1..2,3..4}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1..2 3..4\n");
}

#[test]
fn comma_group_with_leading_dots() {
    let (out, err, code) = run("echo {..1,2}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "..1 2\n");
}

// --- literals (no expansion) ----------------------------------------------

#[test]
fn incomplete_sequences_are_literal() {
    for s in ["{..5}", "{1..}", "{..}", "{a..}"] {
        let (out, err, code) = run(&format!("echo {s}"));
        assert_eq!(code, 0, "stderr={err:?}");
        assert_eq!(out, format!("{s}\n"), "input={s}");
    }
}

#[test]
fn triple_dots_is_literal() {
    let (out, err, code) = run("echo {a..b..c}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "{a..b..c}\n");
}

#[test]
fn overflow_sequence_is_literal() {
    // i64 overflow: the sequence is invalid, so the word stays literal.
    let (out, err, code) = run("echo {1..100000000000000000000}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "{1..100000000000000000000}\n");
}

#[test]
fn min_incr_sequence_is_literal() {
    // An `i64::MIN` increment overflows bash's checked arithmetic either way
    // (descending: no sign flip needed; `start == end`: likewise), so the
    // sequence is invalid and the word stays literal.
    for s in [
        "{5..1..-9223372036854775808}",
        "{5..5..-9223372036854775808}",
    ] {
        let (out, err, code) = run(&format!("echo {s}"));
        assert_eq!(code, 0, "stderr={err:?}");
        assert_eq!(out, format!("{s}\n"), "input={s}");
    }
}

#[test]
fn non_decimal_sequence_is_literal() {
    for s in ["{0x1..0x3}", "{1.5..3}"] {
        let (out, err, code) = run(&format!("echo {s}"));
        assert_eq!(code, 0, "stderr={err:?}");
        assert_eq!(out, format!("{s}\n"), "input={s}");
    }
}

#[test]
fn trailing_junk_sequence_is_literal() {
    // Junk after the end term or the increment term invalidates the
    // sequence; so does mixing an int and a char term.
    for s in ["{1..2x}", "{1..5..2x}", "{a..1}", "{1..a}"] {
        let (out, err, code) = run(&format!("echo {s}"));
        assert_eq!(code, 0, "stderr={err:?}");
        assert_eq!(out, format!("{s}\n"), "input={s}");
    }
}

#[test]
fn single_dot_after_end_term_is_literal() {
    // A single dot after the end term is not a sequence increment (bash:
    // the group stays literal).
    for s in ["{1..5.x3}", "{a..b.x5}"] {
        let (out, err, code) = run(&format!("echo {s}"));
        assert_eq!(code, 0, "stderr={err:?}");
        assert_eq!(out, format!("{s}\n"), "input={s}");
    }
}

#[test]
fn quoted_braces_are_literal() {
    let (out, err, code) = run("echo \"{a,b}\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "{a,b}\n");
}

#[test]
fn partially_quoted_braces_are_literal() {
    let (out, err, code) = run("echo a\"{b,c}\"d");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a{b,c}d\n");
}

#[test]
fn unbalanced_brace_is_literal() {
    let (out, err, code) = run("echo {a");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "{a\n");
    let (out, err, code) = run("echo a}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a}\n");
}

#[test]
fn backslash_in_quotes_shields_brace() {
    // A backslash inside a double-quoted span shields the next byte, so the
    // `{` in it never opens a group; the group after the span still expands.
    let (out, err, code) = run(r#"echo a"x\y"{b,c}"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "ax\\yb ax\\yc\n");
}

// --- opacity of substitution spans and comments ----------------------------

#[test]
fn comment_brace_is_untouched() {
    let (out, err, code) = run("echo a # {b,c}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\n");
}

#[test]
fn command_substitution_result_not_reexpanded() {
    // `$(...)` yields `a b`; brace expansion already ran, so this is literal.
    let (out, err, code) = run("echo $(echo {a,b})");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\n");
}

#[test]
fn function_body_reexpands_per_call() {
    let (out, err, code) = run("f() { echo {a,b}; }; f; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\na b\n");
}

// --- protected contexts ----------------------------------------------------

#[test]
fn assignment_word_is_not_expanded() {
    let (out, err, code) = run("x={a,b}; echo \"$x\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "{a,b}\n");
}

#[test]
fn case_word_and_pattern_are_not_expanded() {
    let (out, err, code) =
        run("case {x,y} in x) echo hit-x;; y) echo hit-y;; *) echo nomatch;; esac");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "nomatch\n");
}

#[test]
fn case_pattern_list_is_not_expanded() {
    let (out, err, code) = run("v=x; case $v in {x,y}) echo matched;; *) echo no;; esac");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "no\n");
}

#[test]
fn here_string_word_is_not_expanded() {
    let (out, err, code) = run("cat <<< {a,b}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "{a,b}\n");
}

#[test]
fn heredoc_body_is_verbatim() {
    let (out, err, code) = run("cat <<EOF\n{a,b}\nEOF");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "{a,b}\n");
}

#[test]
fn heredoc_brace_delimiter_is_verbatim() {
    // A brace-looking delimiter stays literal in the operator and the
    // terminating line (regression: the terminator was brace-expanded and the
    // heredoc never terminated).
    let (out, err, code) = run("cat <<{a,b}\nBODY\n{a,b}");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "BODY\n");
}

// --- word cap (documented deviation) ---------------------------------------

#[test]
fn over_cap_cross_product_is_a_parse_error() {
    // 300 x 300 = 90_000 > MAX_WORDS. Unlike bash (which attempts the
    // allocation and falls back to the literal on failure), fdshell reports a
    // clean parse error.
    let (out, err, code) = run("echo {1..300}{1..300}");
    assert_ne!(code, 0);
    assert!(err.contains("too many words"), "stderr={err:?}");
    assert!(out.is_empty());
}
