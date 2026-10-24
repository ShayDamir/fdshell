#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::keywords::{Closer, Pairing};

struct R {
    end: usize,
    closed: bool,
    quote: bool,
    start: usize,
}

/// [`scan_open`] with the `if` seed every `if … fi` text below scans.
fn scan(line: &[u8], start: usize) -> R {
    scan_open(line, start, Closer::Fi)
}

/// [`scan_block`] seeded with the closer `open`, from a command position: the
/// seed is the closer of the block whose text is scanned (task #309), so a
/// closer of another kind written in that text is word data and never ends the
/// span.
fn scan_open(line: &[u8], start: usize, open: Closer) -> R {
    let mut quotes = Quotes::new();
    let mut s = start;
    let (end, closed) = scan_block(line, start, &mut quotes, &mut s, open, Place::Command);
    R {
        end,
        closed,
        quote: quotes.double,
        start: s,
    }
}

// A comment that contains a closing keyword must not close the block.
#[test]
fn comment_containing_closer_does_not_close() {
    let r = scan(b"if a; then b; # fi", 2);
    assert!(!r.closed, "fi inside # comment must not close the block");
    assert_eq!(r.end, 19);
}

// A single (unclosed) quote must leave in_quote set, and a closer inside the
// unclosed quote must not close the block.
#[test]
fn unclosed_quote_leaves_in_quote() {
    let r = scan(b"if \"a; b; fi", 2);
    assert!(r.quote, "odd number of quotes must leave in_quote true");
    assert!(
        !r.closed,
        "fi inside an open quote must not close the block"
    );
}

// `$var` (no paren) must not open a dollar-paren scope.
#[test]
fn dollar_var_is_not_dollar_paren() {
    let r = scan(b"if $x; then y; fi; echo z", 2);
    assert_eq!(r.end, 18, "fi must close at its own ';'");
    assert!(r.closed);
}

// `$( )` must open and close a dollar-paren scope.
#[test]
fn dollar_paren_single() {
    let r = scan(b"if $(a); then y; fi; echo z", 2);
    assert_eq!(r.end, 20);
    assert!(r.closed);
}

// Nested `$( $( ) )` must track depth.
#[test]
fn dollar_paren_nested() {
    let r = scan(b"if $(a $(b); c); then y; fi; echo z", 2);
    assert_eq!(r.end, 28);
    assert!(r.closed);
}

// A plain top-level `(` is not a dollar-paren scope.
#[test]
fn plain_top_level_paren() {
    let r = scan(b"if (a; b); then y; fi; echo z", 2);
    assert_eq!(r.end, 22);
    assert!(r.closed);
}

// Backticks form a scope; ';' inside is not a separator.
#[test]
fn backtick_scope() {
    let r = scan(b"if `a; b`; then y; fi; echo z", 2);
    assert_eq!(r.end, 22);
    assert!(r.closed);
}

// A double-quoted string with ';' inside is not a separator.
#[test]
fn double_quote_scope() {
    let r = scan(b"if \"a; b\"; then y; fi; echo z", 2);
    assert_eq!(r.end, 22);
    assert!(r.closed);
}

// `;` inside `$( )` is not a top-level separator.
#[test]
fn semicolon_inside_dollar_paren() {
    let r = scan(b"if $(a; b); then y; fi; echo z", 2);
    assert_eq!(r.end, 23);
    assert!(r.closed);
}

// `$var` inside double quotes must not open a dollar-paren scope.
#[test]
fn dollar_var_inside_quotes() {
    let r = scan(b"if \"$x\"; then y; fi; echo z", 2);
    assert_eq!(r.end, 20);
    assert!(r.closed);
}

// `$( )` inside backticks.
#[test]
fn dollar_paren_inside_backtick() {
    let r = scan(b"if `$(a)`; then y; fi; echo z", 2);
    assert_eq!(r.end, 22);
    assert!(r.closed);
}

// A stray `)` at top level is not a dollar-paren closer.
#[test]
fn stray_top_level_closing_paren() {
    let r = scan(b"if a); then y; fi; echo z", 2);
    assert_eq!(r.end, 18);
    assert!(r.closed);
}

// `fi` not followed by a separator must not close the block early.
#[test]
fn closer_without_separator() {
    let r = scan(b"if $(a); then y; fi z; echo w", 2);
    assert_eq!(r.end, 22);
    assert!(r.closed);
}

// Space between `$( )` and the next separator.
#[test]
fn space_after_dollar_paren() {
    let r = scan(b"if $(a) ; then y; fi", 2);
    assert_eq!(r.end, 21);
    assert!(r.closed);
}

// `$( )` inside double quotes is a scope.
#[test]
fn dollar_paren_inside_double_quote() {
    let r = scan(b"if \"a$(b; c)\"; then y; fi; echo z", 2);
    assert_eq!(r.end, 26);
    assert!(r.closed);
}

// Nested `$( )` without a semicolon inside.
#[test]
fn dollar_paren_nested_no_semicolon() {
    let r = scan(b"if $(a $(b)); then y; fi; echo z", 2);
    assert_eq!(r.end, 25);
    assert!(r.closed);
}

// Backtick scope followed by more content on the same line.
#[test]
fn backtick_scope_trailing_content() {
    let r = scan(b"if `a` b; then y; fi; echo z", 2);
    assert_eq!(r.end, 21);
    assert!(r.closed);
}

// Double-quote scope followed by more content on the same line.
#[test]
fn double_quote_scope_trailing_content() {
    let r = scan(b"if \"a\" b; then y; fi; echo z", 2);
    assert_eq!(r.end, 21);
    assert!(r.closed);
}

// A closing keyword inside `$( )` must not close the block.
#[test]
fn closer_inside_dollar_paren() {
    let r = scan(b"if $(a; fi); then y; fi; echo z", 2);
    assert_eq!(r.end, 24);
    assert!(r.closed);
}

// A comment spanning to end-of-line before the real closer.
#[test]
fn comment_before_newline_then_closer() {
    let r = scan(b"if a # c\nthen y; fi", 2);
    assert_eq!(r.end, 20);
    assert!(r.closed);
}

// A space-delimited closer inside `$( )` is data of the inner script and
// must not close the outer block — the block closes at the real `fi`.
#[test]
fn closer_inside_dollar_paren_does_not_close() {
    let r = scan(b"if $( fi ; a); then y; fi; echo z", 2);
    assert!(r.closed, "the outer if must close at the real fi");
    assert_eq!(r.end, 26, "the inner fi must not close at the inner ';'");
}

// A `(` inside double quotes (within a `$( )`) is not a plain paren.
#[test]
fn paren_inside_double_quote_within_dollar_paren() {
    let r = scan(b"if $(x \"a(b\"); then y; fi; echo z", 2);
    assert_eq!(r.end, 26);
}

// A `(` inside backticks (within a `$( )`) is not a plain paren.
#[test]
fn paren_inside_backtick_within_dollar_paren() {
    let r = scan(b"if $(x `a(b`); then y; fi; echo z", 2);
    assert_eq!(r.end, 26);
}

// An unmatched top-level `(` does not open a dollar-paren scope.
#[test]
fn unmatched_top_level_paren() {
    let r = scan(b"if (a; b; then y; fi; echo z", 2);
    assert_eq!(r.end, 21);
}

// A balanced top-level `(...)` mid-line does not open a dollar-paren scope.
#[test]
fn balanced_top_level_paren_mid_line() {
    let r = scan(b"if a (b; c) d; then y; fi; echo z", 2);
    assert_eq!(r.end, 26);
}

// Non-paren content inside `$( )` does not close the scope early.
#[test]
fn content_inside_dollar_paren() {
    let r = scan(b"if $(a b); then y; fi; echo z", 2);
    assert_eq!(r.end, 22);
}

// Backtick scope with a space inside.
#[test]
fn backtick_scope_with_space() {
    let r = scan(b"if `a b` c; then y; fi; echo z", 2);
    assert_eq!(r.end, 23);
}

// A backtick inside double quotes is not a backtick scope.
#[test]
fn backtick_inside_double_quote() {
    let r = scan(b"if \"a`b\"; then y; fi; echo z", 2);
    assert_eq!(r.end, 21);
}

// Two adjacent backtick scopes.
#[test]
fn adjacent_backtick_scopes() {
    let r = scan(b"if `a` `b`; then y; fi; echo z", 2);
    assert_eq!(r.end, 23);
}

// `$( )` immediately followed by a plain paren.
#[test]
fn dollar_paren_then_plain_paren() {
    let r = scan(b"if $(a) (b; c); then y; fi; echo z", 2);
    assert_eq!(r.end, 27);
}

// Nested `$( )` with content and no spaces.
#[test]
fn nested_dollar_paren_compact() {
    let r = scan(b"if $(a$(b);c); then y; fi; echo z", 2);
    assert_eq!(r.end, 26);
}

// A `(` inside double quotes at top level.
#[test]
fn paren_inside_double_quote_top_level() {
    let r = scan(b"if \"a(b\"; then y; fi; echo z", 2);
    assert_eq!(r.end, 21);
}

// A `(` inside backticks at top level.
#[test]
fn paren_inside_backtick_top_level() {
    let r = scan(b"if `a(b`; then y; fi; echo z", 2);
    assert_eq!(r.end, 21);
}

// A plain `(` inside `$( )` adds a depth level, so the following `;` is not
// a top-level separator and the closer is only counted at end of line.
#[test]
fn plain_paren_inside_dollar_paren_adds_depth() {
    let r = scan(b"if $(x (a) fi;", 2);
    assert_eq!(r.end, 15, "plain ( ) must raise dollar-paren depth");
}

// Excess closers in one word run saturate depth at 0 (no u32 underflow).
#[test]
fn excess_closers_saturate_depth() {
    let r = scan(b"if fi fi; then y; fi", 2);
    assert_eq!(r.end, 9);
    assert!(r.closed, "excess closers must saturate depth at 0");
}

// A block-opening keyword inside a double-quoted word is data, not a nested
// block opener (`echo "a for b"` used to leave the block open).
#[test]
fn quoted_keyword_does_not_open_nested_block() {
    let r = scan_open(b"echo \"a for b\"; done", 0, Closer::Done);
    assert!(
        r.closed,
        "for inside double quotes must not count as an opener"
    );
    assert_eq!(r.end, 21);
}

// The same for backtick spans.
#[test]
fn backtick_keyword_does_not_open_nested_block() {
    let r = scan_open(b"echo `a for b`; done", 0, Closer::Done);
    assert!(r.closed, "for inside backticks must not count as an opener");
}

// a quoted `for` inside a `wait` arm used to count as a nested
// block opener, so the `wait` was never seen as closed.
#[test]
fn quoted_keyword_in_wait_arm_does_not_block_closer() {
    let line = b"wait\n  after 1) echo \"idle for a second\" ;;\ndone";
    let r = scan_open(line, 4, Closer::Done);
    assert!(
        r.closed,
        "for inside double quotes must not count as an opener"
    );
}

// Task #236: an unquoted opener in an **argument** position is word data —
// bash recognizes a reserved word in command position only, and `echo "a"
// for b` puts `for` in `echo`'s argument list. The span scan no longer opens
// a nested block there, so the block closes at the `done` (measured against
// `bash -c`: bash rejects the whole shape with rc 2, `syntax error near
// unexpected token 'done'` — the unclosed-construct rc-1-vs-2 class).
#[test]
fn unquoted_keyword_in_an_argument_is_data_the_block_closes() {
    let r = scan_open(b"echo \"a\" for b; done", 0, Closer::Done);
    assert!(
        r.closed,
        "the argument-position `for` must not open a nested block"
    );
    assert_eq!(r.end, 21, "the block closes at its own `done`");
}

// A run that starts already inside an open quote (caller mid-quote) must not
// count a closer that sits in that quoted word, even when the quote never
// closes on this line.
#[test]
fn run_starting_mid_quote_ignores_closer() {
    let mut quotes = Quotes {
        double: true,
        single: false,
    };
    let mut s = 1;
    let (_end, closed) = scan_block(b"\"fi", 1, &mut quotes, &mut s, Closer::Fi, Place::Command);
    assert!(!closed, "fi inside an open quote must not close the block");
    assert!(quotes.double, "the quote must stay open");
}

// A block-opening keyword inside a `$( )` substitution in the body must not
// count: the run keeps going through the substitution, but the words inside
// are data of the inner script, not nested block openers.
#[test]
fn dollar_paren_keyword_does_not_open_nested_block() {
    let r = scan(b"if a; then echo $(echo for); fi", 2);
    assert!(r.closed, "for inside $( ) must not count as an opener");
    assert_eq!(
        r.end, 32,
        "the block must close at the real fi (past the closer)"
    );
}

// Nested substitutions too: every `$( )` level is data.
#[test]
fn nested_dollar_paren_keyword_does_not_open_nested_block() {
    let r = scan(b"if a; then x=$(echo $(echo while)); fi", 2);
    assert!(r.closed, "while inside nested $( ) must not count");
}

// A top-level `(` does not suppress word splits, so the split around `for`
// still happens — and with the command-position rule of task #236 the split
// position settles the place: `x(a` is an argument word, so the `for` behind
// it is argument data and never opens a nested block. The block closes at
// the real `fi` (measured: bash rejects `if x(a for b); then y; fi` with
// rc 2, `syntax error near unexpected token 'a'`; fdshell runs the
// argument-`for` and closes the block, so it accepts at rc 0 — the
// accepts-what-bash-rejects family, not the rc-1-vs-2 one (README
// "Errors").
#[test]
fn plain_paren_does_not_suppress_the_command_position_rule() {
    let r = scan(b"if x(a for b); then y; fi", 2);
    assert!(r.closed, "the argument-position `for` must not count");
    assert_eq!(r.end, 26);
}

// A `)` back at depth 0 restores splits, so a closer after a substitution in
// the same run is *scored* again — and task #309 decides what it closes: the
// scan is seeded with the `if`'s own `fi`, so the `done` is a word and the span
// reaches the real `fi` (the type-blind depth ended the `if` at the `done`,
// end 19). Measured against `bash -c` 5.3.9, `if x=$(a for) done; then y; fi`
// reads `done` as a word too (`done: command not found`, rc 0) — the same
// pairing; the statement's own rc stays the `missing 'then'` divergence.
#[test]
fn closer_after_dollar_paren_in_same_run_counts() {
    let r = scan(b"if x=$(a for) done; then y; fi", 2);
    assert!(
        r.closed,
        "the span closes at its own `fi`, not at the `done`"
    );
    assert_eq!(r.end, 31, "the mismatched `done` never ends the `if` span");
}

// Same split rule for a top-level `(` followed by space and a keyword.
#[test]
fn plain_paren_then_keyword_still_opens() {
    let r = scan(b"if ( for x); then y; fi", 2);
    assert!(!r.closed, "for after a top-level ( ) must still count");
}

// A nested plain `(` inside `$( )` adds a depth level, so no split happens
// around the inner `)` and only the real `fi` closes the block.
#[test]
fn nested_plain_paren_inside_dollar_paren() {
    let r = scan(b"if x=$( (for) done ); then y; fi", 2);
    assert!(r.closed, "the block must close at the real fi");
    assert_eq!(r.end, 33, "done inside the nested ( ) must not count");
}

// A heredoc body line shaped like `fi` does not close the block; only the
// real `fi` does.
#[test]
fn heredoc_body_fi_does_not_close() {
    let r = scan(b"if true; then cat <<EOF\nfi\nEOF\nfi", 2);
    assert!(r.closed, "the block must close at the real fi");
    assert_eq!(r.end, 34);
}

// A `for`-shaped body line (with no `done` in the body) must not open a
// nested depth level.
#[test]
fn heredoc_body_for_does_not_open_depth() {
    let r = scan(b"if true; then cat <<EOF\nfor x in y\nEOF\nfi", 2);
    assert!(
        r.closed,
        "a for-shaped body line must not open a depth level"
    );
    assert_eq!(r.end, 42);
}

// An indented `wait` at the end of a run in a block body opens a nested
// block: the `line`+`base` threading must let the lookahead see the pattern
// keyword on the next line, so the nested `done` closes it and the outer
// `fi` closes the block (a wrong base flips the `wait` to the builtin and
// the block closes early at `done`).
#[test]
fn indented_wait_at_run_end_opens_nested_block() {
    let line = b"if x; then\n  wait\n  readable %rd) y ;;\ndone\nfi";
    let r = scan(line, 2);
    assert!(r.closed, "the outer if must close at the real fi");
    assert_eq!(r.end, 47, "the nested wait must close at its own done");
}

// The same with the pattern keyword on the `wait` line (a mid-run word): the
// split-position lookahead (`base + i`) must land just past `wait`.
#[test]
fn indented_wait_mid_run_opens_nested_block() {
    let line = b"if x; then\n  wait readable %rd) y ;;\ndone\nfi";
    let r = scan(line, 2);
    assert!(r.closed, "the outer if must close at the real fi");
    assert_eq!(r.end, 45, "the nested wait must close at its own done");
}

// A POSIX `wait <pid>` in a block body is the builtin (same-line pid), not a
// nested block opener: the body's `done` must close the block.
#[test]
fn posix_wait_in_body_does_not_open_nested_block() {
    let line = b"wait\n  readable %rd) wait 123 ;;\ndone";
    let r = scan_open(line, 4, Closer::Done);
    assert!(
        r.closed,
        "a same-line pid after wait must not open a nested block"
    );
    assert_eq!(r.end, 38);
}

/// A closer written inside a `'…'` span is data (task #141); the real closer
/// after the span still closes the block.
#[test]
fn single_quoted_closer_is_data() {
    let r = scan(b"if true; then echo 'fi'; fi", 2);
    assert!(r.closed, "the quoted `fi` is data, the real one closes");
    assert_eq!(r.end, 28, "the closer is the last word of the run");
    // A span that hides the only closer leaves the block open.
    let r = scan(b"if true; then echo 'fi'", 2);
    assert!(!r.closed, "the span's `fi` must not close the block");
}

/// A run handed to the scanner while already inside a `'…'` span (the REPL
/// continuation case) keeps the span open and ignores a closer written in it.
#[test]
fn run_starting_mid_span_ignores_closer() {
    let mut quotes = Quotes {
        double: false,
        single: true,
    };
    let mut s = 1;
    let (_end, closed) = scan_block(b"'fi", 1, &mut quotes, &mut s, Closer::Fi, Place::Command);
    assert!(!closed, "fi inside an open span must not close the block");
    assert!(quotes.single, "the span must stay open");
}

/// A run that starts quoted never scores a keyword: `depth_delta`'s first
/// fragment is marked quoted from the caller's state, and a split (the only
/// thing that clears the flag) can only happen unquoted. Each of the three
/// states is pinned on its own — an `&&` in the initial `frag_quoted`
/// expression would silently let one of them score.
#[test]
fn run_starting_quoted_never_scores_a_keyword() {
    let mid_span = Quotes {
        double: false,
        single: true,
    };
    let mid_double = Quotes {
        double: true,
        single: false,
    };
    let mid_backtick = Quotes {
        double: false,
        single: false,
    };
    for (quotes, in_backtick, what) in [
        (mid_span, false, "mid-span"),
        (mid_double, false, "mid-\""),
        (mid_backtick, true, "mid-backtick"),
    ] {
        assert_eq!(
            depth_delta::depth_delta(
                b"fi",
                b"fi",
                0,
                quotes,
                in_backtick,
                Place::Command,
                &mut Pairing::new(),
            ),
            0,
            "{what}: a closer written in a run that starts quoted is data"
        );
    }
}

/// The `'…'` span opened inside a run stays open for the rest of the run (see
/// the `depth_delta` doc): the `done` after the span's closing `'` is **not** a
/// closer — the span swallows the run, which is exactly bash's behaviour for a
/// closer written in the same run as a preceding quoted word (`if true; then
/// echo 'q' done` is a syntax error in bash; fdshell reports the block as
/// unclosed). The pin kills a dropped toggle, which would close the span at
/// the `'` and score the `done`.
///
/// The unquoted twin returns `0` for a **different** cause (task #236): the
/// `done` after a split sits in argument position (behind the command word
/// `echo`), and bash recognizes a closer in command position only —
/// `if true; then echo q done` is a bash syntax error (rc 2) and fdshell
/// reports the block as unclosed too. Keep the two causes separable: the
/// span shape scores 0 *despite* the closer's position, the plain shape
/// scores 0 because the position is an argument.
#[test]
fn span_opened_in_a_run_keeps_the_closer_from_scoring() {
    let line = b"echo 'q' done";
    assert_eq!(
        depth_delta::depth_delta(
            line,
            line,
            0,
            Quotes::new(),
            false,
            Place::Command,
            &mut Pairing::new(),
        ),
        0,
        "the span swallows the rest of the run, so `done` never scores"
    );
    // Without the span the same run scores 0 too, for the *position* reason.
    let line = b"echo q done";
    assert_eq!(
        depth_delta::depth_delta(
            line,
            line,
            0,
            Quotes::new(),
            false,
            Place::Command,
            &mut Pairing::new(),
        ),
        0,
        "the `done` behind a command word is an argument"
    );
    // A run that *starts* at the closer (command position) still scores it.
    let line = b"done";
    assert_eq!(
        depth_delta::depth_delta(
            line,
            line,
            0,
            Quotes::new(),
            false,
            Place::Command,
            &mut Pairing::new(),
        ),
        -1,
        "a closer at the run start is in command position"
    );
}

/// The pair payload rides in the run's **sum** (`delta + payload_delta`), with
/// the payload's own sign and at the place the run has reached when the
/// fragment is read (task #236). The sum is what a block span's depth is built
/// from, so a flipped sign moves a whole block's depth, and a payload scored at
/// the *transition's* place would score in argument position too.
#[test]
fn pair_payload_rides_the_run_sum() {
    // Command position: the glued pair's payload `if` opens, the glued word
    // `a&&if` itself is no keyword, and the block's own words net back to 1.
    let line = b"a&&if :; then :; fi";
    assert_eq!(
        depth_delta::depth_delta(
            line,
            line,
            0,
            Quotes::new(),
            false,
            Place::Command,
            &mut Pairing::new(),
        ),
        1,
        "the payload behind the pair scores once, at the run's current place"
    );
    // A payload that closes carries the closing sign.
    let line = b"a&&done";
    assert_eq!(
        depth_delta::depth_delta(
            line,
            line,
            0,
            Quotes::new(),
            false,
            Place::Command,
            &mut Pairing::new(),
        ),
        -1,
        "a glued closer behind the pair closes the block"
    );
    // Argument position: behind a command word the same payload is data, so the
    // run's sum is 0 — the place is the one the run has reached, not the one
    // `after` hands to the next fragment.
    let line = b"echo a&&if :; then :; fi";
    assert_eq!(
        depth_delta::depth_delta(
            line,
            line,
            0,
            Quotes::new(),
            false,
            Place::Command,
            &mut Pairing::new(),
        ),
        0,
        "a payload behind a command word is an argument"
    );
}

/// [`scan_seed`]: [`scan_block`] with an explicit seed [`Place`], on a `for`
/// block whose closer is `done` (task #306's pins).
fn scan_seed(line: &[u8], start: usize, seed: Place) -> R {
    let mut quotes = Quotes::new();
    let mut s = start;
    let (end, closed) = scan_block(line, start, &mut quotes, &mut s, Closer::Done, seed);
    R {
        end,
        closed,
        quote: quotes.double,
        start: s,
    }
}

/// The block scan's **word-list seed** (task #306): a `for` block's span scan
/// starts just past the opener, so the block's own word list has no opener word
/// for the scorer and the caller seeds the scan with [`Place::Words`]. The
/// closer-shaped list word (`for i in done`) then no longer ends the span — the
/// scan reaches the block's real `done` (end/start `26`, the end of input). The
/// same text with a command seed is the pre-#306 scan: the list word's
/// separator ends the span (end/start `14`).
///
/// And the seed is read by **the run the scan begins in** only: a word list
/// never crosses a separator, so the closer-shaped word of a later run scores
/// and closes the block (last row).
#[test]
fn a_seeded_word_list_span_reaches_the_real_closer() {
    let line = b"for i in done; do :; done";
    let seeded = scan_seed(line, 3, Place::Words);
    assert!(seeded.closed, "the block closes on its own `done`");
    assert_eq!(
        (seeded.end, seeded.start),
        (26, 26),
        "the span covers the whole statement, not the list word's run"
    );
    let unseeded = scan_seed(line, 3, Place::Command);
    assert!(unseeded.closed, "the list word *is* a closer here");
    assert_eq!(
        (unseeded.end, unseeded.start),
        (14, 14),
        "the list word's separator ends the span (the pre-#306 scan)"
    );
    // First-run-only: the seed belongs to the list; every later run starts a
    // command word, so the second run's `done` scores and closes the block.
    let two_runs = scan_seed(b"i in done; done", 0, Place::Words);
    assert!(
        two_runs.closed,
        "the seed reached the run after the separator"
    );
    assert_eq!((two_runs.end, two_runs.start), (16, 16));
}

/// Each caller state keeps a word written **behind** the open scope out of the
/// score too: `bare` is false for the whole run, so the space is no split and the
/// one fragment stays `frag_quoted`, which the split test skips (no `word_step`).
/// This is [`run_starting_quoted_never_scores_a_keyword`] extended to a run with a
/// word after the scope — that pin's one-word run never reaches a split and is
/// therefore blind to `bare`.
///
/// Kills both `&&` of `bare`'s conjunction (`&&`→`||`, measured per mutant): with
/// the first (`!quotes.double`), a word is bare as soon as the double quote is
/// closed; with the second (`!quotes.single`), as soon as the backtick scope is
/// closed. Each makes the space a split, and the quoted fragment is skipped
/// **without** moving `place` (`word_step` is the only thing that moves it), so the
/// closer behind the split is scored at the run's seed [`Place::Command`]: measured
/// `-1` on all three rows under each of the two mutants, where they are `0`.
#[test]
fn a_quoted_run_start_keeps_a_later_word_out_of_the_score() {
    let mid_span = Quotes {
        double: false,
        single: true,
    };
    let mid_double = Quotes {
        double: true,
        single: false,
    };
    let mid_backtick = Quotes::new();
    let in_span: &[u8] = b"'a' fi";
    let in_double: &[u8] = b"\"a\" fi";
    let in_backtick_scope: &[u8] = b"`a` fi";
    for (raw, quotes, in_backtick, what) in [
        (in_span, mid_span, false, "mid-span"),
        (in_double, mid_double, false, "mid-\""),
        (in_backtick_scope, mid_backtick, true, "mid-backtick"),
    ] {
        assert_eq!(
            depth_delta::depth_delta(
                raw,
                raw,
                0,
                quotes,
                in_backtick,
                Place::Command,
                &mut Pairing::new(),
            ),
            0,
            "{what}: a word behind a run that starts quoted is data, never a closer"
        );
    }
}

/// A run whose **first** byte is its scope's closing byte exits the scope: the `"`
/// arm is not gated by `bare`, and the backtick arm is entered from
/// `bare != in_backtick`. The scope ends, the space behind it splits, and the
/// closer scores `-1` at the run's seed [`Place::Command`] — the split skips the
/// quoted fragment without moving `place`.
///
/// Kills the two state-freezing mutants: deleting the `!` of
/// `quotes.double = !quotes.double` leaves the double quote at the caller's value
/// (row 1 measured `0`), and `bare != in_backtick` → `bare == in_backtick` never
/// enters or exits the backtick scope (row 2 measured `0`) — in both, no split ever
/// happens and the run is one quoted fragment.
///
/// Row 3 documents the asymmetry the `depth_delta` doc states: a `'…'` span never
/// closes here, because `bare` is false while the span is open and its closing `'`
/// is gated by `bare`, so the run stays quoted and scores `0`. It is a contrast
/// row, not a toggle kill — the measured `-1` it takes under the two `bare`
/// conjunctions is the same split that [`a_quoted_run_start_keeps_a_later_word_out_of_the_score`]
/// rows pin.
#[test]
fn a_quoted_run_exits_its_scope_at_the_closer_byte() {
    let mid_span = Quotes {
        double: false,
        single: true,
    };
    let mid_double = Quotes {
        double: true,
        single: false,
    };
    let mid_backtick = Quotes::new();
    let double_closer: &[u8] = b"\" fi";
    let backtick_closer: &[u8] = b"` fi";
    let span_closer: &[u8] = b"' fi";
    for (raw, quotes, in_backtick, want, what) in [
        (
            double_closer,
            mid_double,
            false,
            -1,
            "the `\"` closes the caller's double quote",
        ),
        (
            backtick_closer,
            mid_backtick,
            true,
            -1,
            "the backtick closes the caller's scope",
        ),
        (
            span_closer,
            mid_span,
            false,
            0,
            "a `'…'` span never closes, so its run stays quoted",
        ),
    ] {
        assert_eq!(
            depth_delta::depth_delta(
                raw,
                raw,
                0,
                quotes,
                in_backtick,
                Place::Command,
                &mut Pairing::new(),
            ),
            want,
            "{what}"
        );
    }
}

/// A `'…'` span opened **inside** a run swallows every closer behind it (the
/// `depth_delta` doc: this scan never closes a span, since `bare` is false while it
/// is open). Both rows start unquoted, so a split is possible in principle — the
/// toggle is what prevents it. This is
/// [`span_opened_in_a_run_keeps_the_closer_from_scoring`]'s pair: there the closer
/// sits behind a command word (argument position, `0` under both builds), here it
/// sits in command position, so the span is the only thing keeping it out.
///
/// Kills the dropped `!` of `quotes.single = !quotes.single` (the `'` toggle
/// becomes the identity, so a span never opens): every space splits, each fragment
/// before a split carries a quote byte and is skipped with `place` left at the seed,
/// and the closer behind it scores — measured `-1` on both rows where these are
/// `0`. The same rows measure `-1` under the two `bare`-conjunction mutants.
#[test]
fn a_span_opened_in_a_run_swallows_the_closer_behind_it() {
    let opened: &[u8] = b"'a' fi";
    let around_closer: &[u8] = b"'a fi' done";
    for (raw, what) in [
        (opened, "a span opened and left open swallows the closer"),
        (
            around_closer,
            "the span around a closer swallows the word behind it",
        ),
    ] {
        assert_eq!(
            depth_delta::depth_delta(
                raw,
                raw,
                0,
                Quotes::new(),
                false,
                Place::Command,
                &mut Pairing::new(),
            ),
            0,
            "{what}"
        );
    }
}

/// A `(` glued to a word is **word data, not a substitution**: only `$(` raises the
/// substitution depth, so the paren rides in the fragment and `keyword_delta`'s
/// boundary test rejects the keyword-shaped word (`fi(`, `if(`) — the run scores
/// `0`.
///
/// Kills the substitution arm's second `&&` (`&&`→`||`), which degrades the test to
/// "any byte glued to a `(`": the arm then fires on `i(` / `f(`, raises `depth` and,
/// with `step = 2`, eats the `(` out of the fragment, leaving the bare keyword to
/// score at the run's seed — measured `-1` for `fi(` and `+1` for `if(`. The last
/// row is the control, not a kill: a real `$(` is taken identically in both builds
/// (`0` either way) and its fragment `$(fi` is no keyword, so it documents the arm.
#[test]
fn a_glued_open_paren_is_word_data_not_a_substitution() {
    let glued_closer: &[u8] = b"fi(";
    let glued_opener: &[u8] = b"if(";
    let substitution: &[u8] = b"$(fi";
    for (raw, what) in [
        (glued_closer, "a closer glued to a `(` is no closer"),
        (glued_opener, "an opener glued to a `(` is no opener"),
        (substitution, "a real substitution's fragment is no keyword"),
    ] {
        assert_eq!(
            depth_delta::depth_delta(
                raw,
                raw,
                0,
                Quotes::new(),
                false,
                Place::Command,
                &mut Pairing::new(),
            ),
            0,
            "{what}"
        );
    }
}

/// Task #309's seed at the **span** scan: the scan carries the closer of the
/// block whose text it walks, so a closer of another kind written in that text
/// is word data and the span reaches the block's **own** closer. Nested kinds
/// pair innermost-first, a `{ …; }` group nests inside a loop (its `}` pops the
/// group, never the loop), and a `done` of a `case` pattern list mismatches the
/// `case`'s `esac`.
#[test]
fn a_span_pairs_its_closers_by_kind() {
    let lines = [
        // A `while` body: the `fi` is a word, the span ends at the loop's `done`.
        (
            b"while :; do echo if; fi; echo x; done; echo after".as_slice(),
            12,
            Closer::Done,
        ),
        // An `if` body: `done` and `esac` are words, the `fi` ends the span.
        (
            b"if :; then echo a; done; esac; fi; echo after".as_slice(),
            4,
            Closer::Fi,
        ),
        // A group inside a loop body: the `}` pops the group, the `done` the loop.
        (b"while :; do { echo a; } done".as_slice(), 12, Closer::Done),
        // A `case` block: a `done` of a pattern list mismatches, `esac` ends it.
        (b"case x in done) echo p;; esac".as_slice(), 9, Closer::Esac),
        // The `if` inside a `while` body: the `fi` pops the `if`, the span runs
        // on to the loop's `done`.
        (
            b"while :; do if :; then :; fi done".as_slice(),
            12,
            Closer::Done,
        ),
    ];
    let ends = lines.map(|(line, at, open)| {
        let r = scan_open(line, at, open);
        assert!(r.closed, "{line:?} must close");
        r.end
    });
    // Each `end` is one past the block's own closer (the closer plus the
    // separator byte behind it), never one past a foreign closer.
    assert_eq!(ends, [38, 34, 29, 30, 34]);
}

/// The run's sum as `comment.rs::scan_block` takes it: the run is the whole
/// input, the caller's quote state is empty, the run starts at a command word.
fn delta(raw: &[u8]) -> i32 {
    depth_delta::depth_delta(
        raw,
        raw,
        0,
        Quotes::new(),
        false,
        Place::Command,
        &mut Pairing::new(),
    )
}

/// The substitution-depth arms of `depth_delta` decide which spaces split the
/// run, and a split is what puts a word in command position, so every arm is
/// pinned on its own: the three mutations of that pair of counters
/// (`$(`-detection, the nested `(`, and the bare-`$` guard) are each invisible
/// to the other two, and all three leave the run scorable through a split.
///
/// The shared shape is a `$( … )` fragment followed by a keyword: a substitution
/// fragment is no command word, so the word behind it is still in command
/// position and scores (unlike the word behind a command word, which the
/// argument-position rule already pins).
#[test]
fn substitution_depth_gates_which_spaces_split_the_run() {
    // `$( … )` opens the depth at the `$(` pair, so the spaces inside the
    // substitution are data and the `if` written behind it is the run's next
    // word in command position: +1.
    assert_eq!(
        delta(b"$( a ) if"),
        1,
        "the substitution's own spaces never split, and its word keeps command position"
    );
    // A nested `(` inside the substitution raises the depth one more, so the
    // first `)` closes the *nested* level: the substitution is still open at the
    // end of the run and the `if` is its data.
    assert_eq!(
        delta(b"$( ( ) if"),
        0,
        "the nested paren is closed by the first `)`, the substitution stays open"
    );
    // A bare `$x` opens no depth (its next byte is no `(`), so it is an ordinary
    // command word: the pair behind it is scored in the argument position the
    // word established, not in command position.
    assert_eq!(
        delta(b"$( a ) $x b&&if"),
        0,
        "a bare `$` is a word, so the glued payload behind it is an argument"
    );
}
