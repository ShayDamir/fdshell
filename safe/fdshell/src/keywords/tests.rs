#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

#[test]
fn keyword_delta_closer_with_extra_boundary() {
    assert_eq!(keyword_delta(b"fi|", b"fi|", 3), Some(-1));
    assert_eq!(keyword_delta(b"done&", b"done&", 5), Some(-1));
    assert_eq!(keyword_delta(b"esac|", b"esac|", 5), Some(-1));
}

#[test]
fn keyword_delta_opener_with_semi_boundary() {
    assert_eq!(keyword_delta(b"if;", b"if;", 3), Some(1));
    assert_eq!(keyword_delta(b"for;", b"for;", 4), Some(1));
}

/// The `wait` lookahead: a word on a subsequent line, or a same-line pattern
/// keyword, opens a block; a same-line pid/`$!`/name, a `;`, a quoted word,
/// or EOF is the POSIX builtin.
#[test]
fn wait_opens_block_cases() {
    assert!(wait_opens_block(b"wait\n readable %rd) x ;;", 4));
    assert!(!wait_opens_block(b"wait 123", 4));
    assert!(!wait_opens_block(b"wait", 4));
    assert!(!wait_opens_block(b"wait \"readable\"", 4));
    // A `;` after `wait` is a statement separator, not a block opener.
    assert!(!wait_opens_block(b"wait ; readable", 4));
    assert!(!wait_opens_block(b"wait; echo hi", 4));
    assert!(wait_opens_block(b"wait # c\nreadable", 4));
    assert!(wait_opens_block(b"wait after 5)", 4));
    // A same-line non-keyword (pid, `$!`, or a name) is the builtin.
    assert!(!wait_opens_block(b"wait 123abc", 4));
    assert!(!wait_opens_block(b"wait $!", 4));
    assert!(!wait_opens_block(b"wait readableX", 4));
    // A word on a subsequent line is always a block arm.
    assert!(wait_opens_block(b"wait\ndone", 4));
    assert!(wait_opens_block(b"wait\nsleeping", 4));
}

/// The lookahead edge cases a single-byte mutation would flip: a bare
/// `wait\n` (EOF after the newline) is the builtin, a `;` after the newline
/// is skipped (the subsequent-line word still opens a block), and a quoted
/// word on a subsequent line is data.
#[test]
fn wait_opens_block_lookahead_edges() {
    assert!(!wait_opens_block(b"wait\n", 4));
    assert!(wait_opens_block(b"wait\n; readable", 4));
    assert!(!wait_opens_block(b"wait\n\"readable\"", 4));
    assert!(!wait_opens_block(b"wait\n'readable'", 4));
}

/// `keyword_delta` for `wait` is `Some(1)` for the event-case block and `None`
/// for the POSIX builtin (task #309): the recognition table [`keyword_kind`]
/// answers *which keyword is this*, and the builtin is **no keyword** at all.
/// The pre-#309 table scored it `Some(0)`; every consumer reads the two the
/// same (`scores`' `_` arm, `.unwrap_or(0)`, `is_some_and(|d| d < 0)`,
/// `segment::block::midrun`'s `== Some(1)` gate), which is why this is the only
/// pin the projection moves.
#[test]
fn keyword_delta_wait_block_vs_builtin() {
    assert_eq!(keyword_delta(b"wait", b"wait\n readable", 4), Some(1));
    assert_eq!(keyword_delta(b"wait", b"wait 123", 4), None);
    assert_eq!(keyword_delta(b"wait", b"wait", 4), None);
}

/// `first_word_end` stops at the first whitespace/`;`.
#[test]
fn first_word_end_stops_at_separator() {
    assert_eq!(first_word_end(b"wait 123", 0), 4);
    assert_eq!(first_word_end(b"wait;done", 0), 4);
    assert_eq!(first_word_end(b"wait", 0), 4);
    assert_eq!(first_word_end(b"wait x", 2), 6);
}

/// `{`/`}` are the group command's delimiters: a `{` with a boundary after it
/// (whitespace, `&`, `|`, end of input) is `+1`, `}` with a boundary (incl. the
/// glued redirect forms) is `-1`, and a glued brace word is neither.
#[test]
fn keyword_delta_group_delimiters() {
    assert_eq!(keyword_delta(b"{", b"{ echo a; }", 1), Some(1));
    assert_eq!(keyword_delta(b"{ echo a; }", b"{ echo a; }", 8), Some(1));
    assert_eq!(keyword_delta(b"{\n", b"{\n echo a", 1), Some(1));
    assert_eq!(keyword_delta(b"{&x", b"{&x", 2), Some(1));
    assert_eq!(keyword_delta(b"{|x", b"{|x", 2), Some(1));
    // A glued brace word (command name or brace expansion) is not an opener.
    assert_eq!(keyword_delta(b"{a,b}", b"{a,b}", 5), None);
    assert_eq!(keyword_delta(b"{1..5}", b"{1..5}", 6), None);
    assert_eq!(keyword_delta(b"{echo", b"{echo hi; }", 5), None);
    assert_eq!(keyword_delta(b"{}", b"{}; echo hi", 2), None);
    assert_eq!(keyword_delta(b"{;", b"{; echo a; }", 2), None);
    // A bare `{` glued to `;` is the word `{;`, so the line byte rules it out.
    assert_eq!(keyword_delta(b"{", b"{; echo a; }", 1), None);
    // Closers, including the glued fd-qualified redirect forms.
    assert_eq!(keyword_delta(b"}", b"{ echo a; }", 11), Some(-1));
    assert_eq!(keyword_delta(b"};", b"{ echo a; };", 2), Some(-1));
    assert_eq!(keyword_delta(b"}>out", b"{ echo a; }>out", 10), Some(-1));
    assert_eq!(keyword_delta(b"}2>err", b"{ echo a; }2>err", 10), Some(-1));
    assert_eq!(keyword_delta(b"}echo", b"}echo", 5), None);
    assert_eq!(keyword_delta(b"}a", b"}a", 2), None);
}

/// The `time [-p]` prefix: transparent for the depth rule, so the keyword
/// behind it is scored and the statement stays one segment.
#[test]
fn time_prefix_is_transparent_for_the_depth_rule() {
    assert_eq!(time_prefix_len(b"time while false"), Some(5));
    assert_eq!(time_prefix_len(b"time -p while false"), Some(8));
    assert_eq!(time_prefix_len(b"time  -p { echo a; }"), Some(9));
    assert_eq!(time_prefix_len(b"while false"), None);
    assert_eq!(time_prefix_len(b"timeless"), None);
    assert_eq!(time_prefix_len(b"time -px"), None);
    assert_eq!(time_prefix_len(b"timefoo"), None);

    assert_eq!(
        keyword_delta(b"time while false", b"time while false", 4),
        Some(1)
    );
    assert_eq!(
        keyword_delta(
            b"time -p case x in x) echo;; esac",
            b"time -p case x in x) echo;; esac",
            4
        ),
        Some(1)
    );
    assert_eq!(keyword_delta(b"timeless x", b"timeless x", 4), None);
}

/// Repeated prefixes all count, and a bare `time` / `time -p` is a prefix of
/// its own length (the group scanner trims the run before the brace).
#[test]
fn time_prefix_run_counts_all_prefixes() {
    assert_eq!(time_prefix_len(b"time"), Some(4));
    assert_eq!(time_prefix_len(b"time -p"), Some(7));
    assert_eq!(time_prefix_len(b"time time -p ls"), Some(13));
    assert_eq!(
        keyword_delta(b"time time -p while x", b"time time -p while x", 4),
        Some(1)
    );
}

/// The length ends exactly at the word behind the run: the prefix's own
/// whitespace (between `time` and `-p`, and closing the run) is counted once, by
/// the loop — no term is redundant, so each one is pinned here.
#[test]
fn time_prefix_length_ends_at_the_word_behind_it() {
    assert_eq!(time_prefix_len(b"time ls"), Some(5));
    assert_eq!(time_prefix_len(b"time  ls"), Some(6));
    assert_eq!(time_prefix_len(b"time -p ls"), Some(8));
    assert_eq!(time_prefix_len(b"time \t-p  ls"), Some(10));
    assert_eq!(time_prefix_len(b"time time"), Some(9));
    assert_eq!(time_prefix_len(b"time time -p"), Some(12));
    // A glued `-p` is the flag of a glued prefix: still transparent.
    assert_eq!(time_prefix_len(b"time-p ls"), Some(7));
    // Not prefixes: a longer word, a glued flag word, a flag without a `time`.
    assert_eq!(time_prefix_len(b"timely"), None);
    assert_eq!(time_prefix_len(b"time-px ls"), None);
    assert_eq!(time_prefix_len(b"-p ls"), None);
}

/// Closers are transparent too: `time` in front of a closer scores the closer,
/// so the group scanner and the block scan see the same keyword.
#[test]
fn time_prefix_in_front_of_a_closer_scores_the_closer() {
    assert_eq!(keyword_delta(b"time }", b"time }", 5), Some(-1));
    assert_eq!(keyword_delta(b"time -p }", b"time -p }", 8), Some(-1));
    assert_eq!(keyword_delta(b"time fi", b"time fi", 4), Some(-1));
    assert_eq!(keyword_delta(b"time done&", b"time done&", 4), Some(-1));
    assert_eq!(keyword_delta(b"time fi|", b"time fi|", 4), Some(-1));
    assert_eq!(keyword_delta(b"time esac|", b"time esac|", 4), Some(-1));
}

/// Nested groups net out of the depth, so an inner group never closes an
/// enclosing keyword block.
#[test]
fn keyword_delta_group_nests_to_zero() {
    let line = b"{ { echo n; }; }";
    let mut depth = 0;
    for word in [b"{", b"{", b"}", b"}"] {
        depth += keyword_delta(word, line, word.len()).unwrap_or(0);
    }
    assert_eq!(depth, 0);
}

/// A word that contains quote bytes is data (task #141): `"fi"`, `'done'`, `'{'`
/// are command words, so they open and close no block — as bash does.
#[test]
fn quoted_keyword_words_are_data() {
    assert!(!quoted_word(b"fi|", 2), "the bare keyword is unquoted");
    assert!(quoted_word(b"\"fi\"", 4));
    assert!(quoted_word(b"'fi'", 4));
    assert!(quoted_word(b"'done'", 6));
    // A quote byte anywhere in the word counts: `x'fi`.
    assert!(quoted_word(b"x'fi", 4));
    assert_eq!(keyword_delta(b"'fi'", b"'fi'|", 4), None);
    assert_eq!(keyword_delta(b"\"fi\"", b"\"fi\"|", 5), None);
    assert_eq!(keyword_delta(b"'done'", b"'done'&", 6), None);
    assert_eq!(keyword_delta(b"'{'", b"'{' x }", 3), None);
    // The unquoted keywords behind them still score, so the rule is a gate and
    // not a blanket `None`.
    assert_eq!(keyword_delta(b"fi", b"fi|", 2), Some(-1));
    assert_eq!(keyword_delta(b"{", b"{ x }", 1), Some(1));
}

/// The gate's span rule, pinned at the byte level for the spellings the
/// parse layer gates through `parse::word_quoted::keyword_word` (task #247):
/// the quote bytes of the word's own run decide, and the scan stops at the
/// word break, so a *preceding* quoted word never taints the next word.
#[test]
fn quoted_word_reads_the_word_span_not_the_line() {
    assert!(!quoted_word(b"if x", 2));
    assert!(quoted_word(b"'if' x", 4));
    assert!(quoted_word(b"\"if\" x", 4));
    assert!(quoted_word(b"i\"f\" x", 4));
    // The empty pairs have no mask bit on the keyword bytes — only the span
    // sees them (bash: both are command words).
    assert!(quoted_word(b"\"\"if x", 4));
    assert!(quoted_word(b"''if x", 4));
    assert!(quoted_word(b"\"i\"f x", 4));
    // The scan stops at the word break: the `if` after a quoted word is
    // unquoted on its own span.
    assert!(!quoted_word(b"echo \"a\" if", 12));
    // A quoted keyword word scores no keyword delta, the unquoted one does.
    assert_eq!(keyword_delta(b"\"\"if", b"\"\"if x", 4), None);
    assert_eq!(keyword_delta(b"if", b"if x", 2), Some(1));
}
