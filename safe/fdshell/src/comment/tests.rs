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
    let (end, closed, _) = scan_block(line, start, &mut quotes, &mut s, open, Place::Command);
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
    let (_end, closed, _) = scan_block(b"\"fi", 1, &mut quotes, &mut s, Closer::Fi, Place::Command);
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
    let (_end, closed, _) = scan_block(b"'fi", 1, &mut quotes, &mut s, Closer::Fi, Place::Command);
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

/// A `'…'` span opened inside a run closes at its closing `'` (task #519), so
/// the completed quoted word splits like any other word: the word-start quote
/// gate scores it `0` — word data, never a keyword — and it puts the closer
/// behind it in **argument** position, so the run scores `0`. That is the same
/// cause as the unquoted twin below, and it is bash's reason (`if true; then
/// echo 'q' done` and `if true; then echo q done` are both syntax errors in
/// bash, rc 2; fdshell reports the block as unclosed). The test keeps the name
/// it carried as the `'` toggle's kill pin; that kill is
/// [`a_quoted_run_exits_its_scope_at_the_closer_byte`]'s now (task #519).
///
/// The unquoted twin returns `0` for that same position reason (task #236): the
/// `done` after a split sits in argument position (behind the command word
/// `echo`), and bash recognizes a closer in command position only. Keep the
/// rows' causes stated: both are the word gate + argument position now.
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
        "the quoted word scores 0 at the word gate and leaves `done` an argument"
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
    let (end, closed, _) = scan_block(line, start, &mut quotes, &mut s, Closer::Done, seed);
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
/// score too: the run's first byte closes the caller's scope (all three arms
/// close it — task #519 made the `'` arm symmetric with the `"` arm, and the
/// backtick arm always entered from `bare != in_backtick`), the word's own
/// closing pair reopens its scope before the space, so the space is no split
/// and the run stays one `frag_quoted` fragment — the **run-start** flag is
/// what skips it (no `word_step`) at the run's end. This is
/// [`run_starting_quoted_never_scores_a_keyword`] extended to a run with a
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

/// A run whose **first** byte is its scope's closing byte exits the scope: neither
/// quote arm is gated by `bare` (task #519 made the `'` arm symmetric with the `"`
/// arm), and the backtick arm is entered from `bare != in_backtick`. The scope
/// ends, the space behind it splits, and the closer scores `-1` at the run's seed
/// [`Place::Command`] — the split skips the quoted fragment without moving `place`.
///
/// Kills the three state-freezing mutants: deleting the `!` of each toggle leaves
/// the quote at the caller's value — row 1 (`"`) and row 3 (`'`) each measure `0`
/// under their mutant — and `bare != in_backtick` → `bare == in_backtick` never
/// enters or exits the backtick scope (row 2 measured `0`) — in each, no split ever
/// happens and the run is one quoted fragment. The two `bare` conjunctions stay
/// pinned by [`a_quoted_run_start_keeps_a_later_word_out_of_the_score`].
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
            -1,
            "the `'` closes the caller's span, like the other two",
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

/// A `'…'` span opened **inside** a run closes at its closing `'` (task #519),
/// so the quoted word splits at the space behind it: the word-start quote gate
/// scores the word `0` — it is word data — and the closer written after it
/// lands in argument position, so both rows score `0`. This is
/// [`span_opened_in_a_run_keeps_the_closer_from_scoring`]'s pair: there the
/// closer sits behind a command word, here the quoted word itself takes the
/// command position (the run start) and the closer behind it scores `0`.
///
/// The test used to kill the dropped `!` of `quotes.single = !quotes.single`
/// (the toggle becoming the identity, so a span never opened). On the fixed
/// machine that mutant answers `0` for both these rows as well — every
/// fragment goes through the word gate and leaves `place` at argument position
/// — and the toggle kill is now
/// [`a_quoted_run_exits_its_scope_at_the_closer_byte`]'s (task #519).
#[test]
fn a_span_opened_in_a_run_swallows_the_closer_behind_it() {
    let opened: &[u8] = b"'a' fi";
    let around_closer: &[u8] = b"'a fi' done";
    for (raw, what) in [
        (
            opened,
            "the closed span's word is scored 0 by the word gate",
        ),
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

/// A `(` glued to a word **ends the keyword word** (task #396, the one
/// word-break set): `fi(` and `if(` are scored, `-1` and `+1` — bash reads both
/// as keywords at its metacharacter boundary and only the grammar rejects them
/// (`fi(x)` and `if(x)` are syntax errors, measured). The test keeps its name
/// for the substitution arm it was built to kill; that arm's old kill is gone —
/// with the boundary rule accepting the glued keyword, good and mutant builds
/// agree on the first two rows, so they now pin the rule instead.
///
/// The last row is the control the entry measured: a real `$(` raises the
/// substitution depth and its fragment `$(fi` is no keyword (the word starts at
/// the `$`), so the run scores `0`.
#[test]
fn a_glued_open_paren_is_word_data_not_a_substitution() {
    let glued_closer: &[u8] = b"fi(";
    let glued_opener: &[u8] = b"if(";
    let substitution: &[u8] = b"$(fi";
    for (raw, want, what) in [
        (
            glued_closer,
            -1,
            "a closer glued to a `(` is its keyword (task #396)",
        ),
        (
            glued_opener,
            1,
            "an opener glued to a `(` is its keyword (task #396)",
        ),
        (
            substitution,
            0,
            "a real substitution's fragment is no keyword",
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
            want,
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

/// The two substitution-depth guards of `depth_delta` decide which spaces split
/// the run, and a split is what puts a word in command position, so each of
/// their four mutants is pinned on its own: `depth_delta.rs:126`'s
/// `$(`-detection (the `i + 1` offset and the `== Some(&b'(')`) and `:129`'s
/// nested paren (the `b == b'('` and the `depth > 0`). A mutant of one guard is
/// invisible to the other, and all four leave the run scorable through a split.
///
/// The shared shape is a `$( … )` fragment followed by a keyword: a substitution
/// fragment ends with `)`, which restarts command position, so the word behind
/// it scores (unlike the word behind a command word, which the
/// argument-position rule already pins). The rows and the mutants they kill,
/// with the values measured on the clean tree: row 0 `+1` kills the nested
/// paren's `==`→`!=`, row 1 `0` kills the nested paren's `depth > 0`→`depth < 0`
/// and both `$(`-detection mutants, row 2 `0` kills the `$(`-detection
/// `==`→`!=` a second way.
#[test]
fn substitution_depth_gates_which_spaces_split_the_run() {
    // `$( … )` opens the depth at the `$(` pair, so the spaces inside the
    // substitution are data and the `if` written behind it is the run's next
    // word in command position: +1. The nested paren's `b == b'('` →
    // `b != b'('` fails here: every byte inside raises the depth, the run never
    // splits, and the `if` scores nothing. The `$(`-detection mutants do NOT
    // show here: they degrade the run to the `$(`, `a` and `)` fragments, and
    // the `)` restarts command position, so the row keeps its clean +1.
    assert_eq!(
        delta(b"$( a ) if"),
        1,
        "the substitution's own spaces never split, and its word keeps command position"
    );
    // A nested `(` inside the substitution raises the depth one more, so the
    // first `)` closes the *nested* level: the substitution is still open at the
    // end of the run and the `if` is its data. Three mutants turn this row into
    // +1: the nested arm's `depth > 0` (the arm never fires, so the first `)`
    // closes the substitution) and the `$(` guard's two mutants — the offset
    // `i + 1` → `i * 1` and the `== Some(&b'(')` (no substitution opens, every
    // space splits, and the `(`/`)` fragments restart command position).
    assert_eq!(
        delta(b"$( ( ) if"),
        0,
        "the nested paren is closed by the first `)`, the substitution stays open"
    );
    // A bare `$x` opens no depth (its next byte is no `(`), so it is an ordinary
    // command word: the pair behind it is scored in the argument position the
    // word established, not in command position. Inverting the guard's `==`
    // opens a depth on the bare `$`, so the run never splits and the glued
    // payload is scored at the substitution's command position: +1.
    assert_eq!(
        delta(b"$( a ) $x b&&if"),
        0,
        "a bare `$` is a word, so the glued payload behind it is an argument"
    );
}

/// Task #513 (rule 9): the heredoc branch of [`scan_block`] (`i = resume;
/// *start = i + 1;`) is asserted at the exact boundary. When the run's own
/// closer keyword closes the pairing, the loop exits right after the branch,
/// so the assignment — not the later `else`-arm recomputation — is the final
/// state. The entry's old equivalence proof ("the `else` arm resets
/// `*start` … before any later run is measured") holds only while the loop
/// keeps going; on this shape the answer is assertable, so both recorded
/// mutants (`+`→`-`, `+`→`*`) die on the exact values below (measured on the
/// clean tree).
#[test]
fn the_heredoc_branch_records_start_past_the_delimiter_newline() {
    let r = scan(b"fi a <<X\nX\ny", 0);
    assert_eq!(
        r.end, 10,
        "resume is the newline terminating the delimiter line"
    );
    assert!(r.closed, "the run's own `fi` closes the pairing");
    assert!(!r.quote);
    assert_eq!(r.start, 11, "the branch advances start to resume + 1");
}

/// [`depth_delta`] on the whole run at the place `place`, with the pairing seeded
/// with `open` — task #309's seed: the closer of the block whose text is scanned
/// (`None` keeps the top-level clamp the run's sum is normally read through).
fn delta_seeded(raw: &[u8], place: Place, open: Option<Closer>) -> i32 {
    let mut pairing = Pairing::new();
    if let Some(closer) = open {
        pairing.open(closer);
    }
    depth_delta::depth_delta(raw, raw, 0, Quotes::new(), false, place, &mut pairing)
}

/// Task #421: the byte-run scorer is the one scorer whose word unit is coarser than
/// the tokenizer's, so the closer glued behind a `case` pattern word's `)` is read
/// from **inside** the word (`keywords/position/paren.rs`), at the word's own place
/// and through the existing gates. Every value is measured on the clean tree: the
/// payload scores at a command position and at the `case` list's `Pattern` place, the
/// `for` **word-list** place keeps it list data (task #306), a foreign-kind closer
/// glued behind the `)` stays word data (task #309's kind gate), and a word the
/// word-start row recognizes gets **one** score — the row owns it, so the payload
/// adds nothing (without that rule `esac)esac` pops twice). An arithmetic word's `)`
/// are region bytes (task #396's carve-out) and a quoted payload is answered by the
/// recognition table's quote gate (task #413).
#[test]
fn a_closer_behind_a_pattern_paren_is_scored_through_the_gates() {
    for place in [Place::Command, Place::Pattern] {
        assert_eq!(
            delta_seeded(b"a)esac", place, Some(Closer::Esac)),
            -1,
            "{place:?}"
        );
        assert_eq!(
            delta_seeded(b"b)esac>f", place, Some(Closer::Esac)),
            -1,
            "{place:?}"
        );
    }
    assert_eq!(
        delta_seeded(b"a)esac", Place::Words, Some(Closer::Esac)),
        0,
        "a word of a `for` list scores nothing"
    );
    assert_eq!(
        delta_seeded(b"a)fi", Place::Command, Some(Closer::Esac)),
        0,
        "a closer of another kind is word data"
    );
    assert_eq!(
        delta_seeded(b"done)esac", Place::Pattern, Some(Closer::Esac)),
        -1,
        "the word-start `done` is word data here, so the payload pops the `case`"
    );
    assert_eq!(
        delta_seeded(b"esac)esac", Place::Pattern, Some(Closer::Esac)),
        -1,
        "one score per word: the word-start row owns this word"
    );
    assert_eq!(
        delta_seeded(b"((x=0;x<2;x++))esac", Place::Command, Some(Closer::Esac)),
        0,
        "the arithmetic region's `)` are region bytes"
    );
    assert_eq!(
        delta_seeded(b"a)\"esac\"", Place::Command, Some(Closer::Esac)),
        0,
        "a quoted payload is data — the quotes are the operand's"
    );
}

/// Task #421's scan-level shape: the glued closer closes the span it was seeded
/// for, from the first run of the block's own text (the scan starts just past the
/// opener, task #306's seed paragraph). The multi-clause spelling puts the glued
/// closer in the run **behind** a `;;`, where every run starts at a command
/// position again.
#[test]
fn a_glued_closer_closes_the_span_it_was_seeded_for() {
    for line in [
        b"case x in a)esac".as_slice(),
        b"case x in a) echo hi;;b)esac",
        b"case abc in (a*)esac",
    ] {
        let r = scan_open(line, 5, Closer::Esac);
        assert!(r.closed, "{line:?} must close");
    }
    // The kind gate at work inside a scan: the `done)` head is word data (the
    // innermost block wants `esac`), the glued `esac` pops the `case`, and the
    // loop's own `done` pops the `while` — so the span reaches the loop's closer,
    // which is the last word of the line and leaves the scan one past its end.
    let line = b"while :; do case x in done)esac; break; done";
    let r = scan_open(line, 6, Closer::Done);
    assert!(r.closed, "{line:?} must close");
    assert_eq!(
        r.end,
        line.len() + 1,
        "the span runs to the loop's own `done`, the scan's last word is the closer"
    );
}

/// One score per word, asserted where the pairing can see it: a word whose own
/// start is a keyword that **scores** contributes a single pop
/// (`keywords/position/paren.rs`'s guard), while a word whose word-start keyword
/// the kind gate rejects — word data — hands its score to the glued payload.
#[test]
fn the_word_start_row_owns_a_word_it_recognizes() {
    // The word-start row fires (the run starts at a command position and nothing
    // is open, so the stray-closer clamp answers): one pop, never two.
    assert_eq!(delta(b"esac)esac"), -1, "one score per word");
    assert_eq!(delta(b"done)esac"), -1, "one score per word");
    // A word the rows leave word data: the payload is its only score.
    assert_eq!(delta(b"a)esac"), -1, "the payload is the only score");
    assert_eq!(
        delta(b"a)"),
        0,
        "a `)` that ends the word carries no payload"
    );
    assert_eq!(delta(b"esac"), -1, "the plain closer, unchanged");
}

/// Task #436's record, read at the **top level** of a scan — an empty pairing —
/// where a closer word in command position that pairs with nothing is recorded
/// once, at its own offset, and the score is untouched. Both sides of every gate
/// are pinned: the place gate answers first (an argument word is never asked, and
/// `keywords/position` answers before the decision is asked); a scan seeded with
/// the block it is inside records nothing, because a mismatch there only says
/// *this scanner* did not pair the word (the glued-`|`, cond-pair and `case`
/// word-slot rows of `tests/cond_blocks.rs`, all bash 5.3.9 rc 0); the brace
/// closer is `segment/group.rs`'s and `parse/function_block.rs`'s own and is
/// never asked; and a scorer that walks text it will never run simply never reads
/// the record, so the decision costs it nothing.
#[test]
fn the_scan_records_a_top_level_closer() {
    // A closer word at a run start, nothing open: recorded at its own offset.
    let mut p = Pairing::new();
    assert_eq!(
        depth_delta::depth_delta(
            b"done",
            b"done",
            0,
            Quotes::new(),
            false,
            Place::Command,
            &mut p,
        ),
        -1,
        "a closer at a run start is in command position — the score is unchanged"
    );
    assert_eq!(p.stray(), Some(0), "the word is recorded at its own offset");

    // The place gate answers first: an argument word is never asked.
    let mut p = Pairing::new();
    assert_eq!(
        depth_delta::depth_delta(
            b"echo fi",
            b"echo fi",
            0,
            Quotes::new(),
            false,
            Place::Command,
            &mut p,
        ),
        0,
        "an argument word scores nothing"
    );
    assert_eq!(p.stray(), None, "an argument is not a stray");

    // Seeded with the block the caller is inside: the mismatch arm scores 0 and
    // records nothing — bash runs those lines.
    let mut seeded = Pairing::new();
    seeded.open(Closer::Done);
    assert_eq!(
        depth_delta::depth_delta(
            b"fi",
            b"fi",
            0,
            Quotes::new(),
            false,
            Place::Command,
            &mut seeded,
        ),
        0,
        "a mismatched closer is word data (task #309)"
    );
    assert_eq!(seeded.stray(), None, "a seeded scan records nothing");

    // The brace closer keeps its score and is never asked.
    let mut p = Pairing::new();
    assert_eq!(
        depth_delta::depth_delta(b"}", b"}", 0, Quotes::new(), false, Place::Command, &mut p,),
        -1,
        "the brace's own score at the top level"
    );
    assert_eq!(p.stray(), None, "the brace belongs to the group scanner");

    // A `time [-p]` prefix run in front of the closer is one word for the walk,
    // and the keyword behind it is what the recognition table reads — so the
    // closer word records its own start, and the report names `fi` and not `time`.
    let mut p = Pairing::new();
    assert_eq!(
        depth_delta::depth_delta(
            b"time fi",
            b"time fi",
            0,
            Quotes::new(),
            false,
            Place::Command,
            &mut p,
        ),
        -1,
        "the prefix is transparent — the score is the closer's own"
    );
    assert_eq!(
        p.stray(),
        Some(5),
        "the closer word records its own start — a `time` prefix run alone resolves to no keyword"
    );
}

/// Task #520: the run's word unit is a space **or a tab** — the two bytes bash
/// breaks a word on that can sit *inside* a run. `case x in` + tab + `esac` is
/// the three words `x`, `in`, `esac`; `in` opens the clause list and POSIX
/// #2.9.1 makes the clause list optional, so the block closes at the `esac`
/// written in list position (bash rc 0; the end-to-end pin is
/// `tests/case.rs::the_tab_spelling_of_the_empty_clause_list_is_the_space_one`).
/// Each row is the kill row of one side of the arm: the tab row kills the tab
/// test and the `||`, the space row kills the space test and the `||` (a byte is
/// only ever one of the two, so the `&&` mutant splits nothing at all), and the
/// `<VT>` row is the control that the set is not "any whitespace".
#[test]
fn the_run_splits_its_words_on_a_space_or_a_tab() {
    for raw in [b"case x in\tesac".as_slice(), b"case x in esac"] {
        let r = scan_open(raw, 5, Closer::Esac);
        assert!(r.closed, "{raw:?}: the closer is a word of its own");
    }
    // Bash refuses the `<VT>` row too (rc 2, here rc 1), so the byte stays word
    // data and the run never reaches the closer as a word.
    let raw: &[u8] = b"case x in\x0besac";
    assert!(
        !scan_open(raw, 5, Closer::Esac).closed,
        "the set is the two bytes, not every whitespace byte"
    );
}
