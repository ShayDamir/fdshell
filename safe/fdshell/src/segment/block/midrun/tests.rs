#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::keywords::keyword_delta;

/// Score the word at `at` as `skip` does: `Some(resume)` when the word opens a
/// mid-run cond-part block, `None` when it opens none.
fn skip_at(line: &[u8], at: usize) -> Option<usize> {
    let mut book = Book::new();
    book.start = 0;
    book.i = at;
    skip(line, &mut book)
}

/// The book's mid-run flag after `skip` has taken (or refused) the word at
/// `at`: the REPL's continuation signal, set only by an unclosed span.
fn flag_at(line: &[u8], at: usize) -> bool {
    let mut book = Book::new();
    book.start = 0;
    book.i = at;
    skip(line, &mut book);
    book.midrun_open
}

/// The flag for a word written after `x && ` (mid-run, `group_at_run` ==
/// `Some(false)`).
fn flag(line: &[u8]) -> bool {
    flag_at(line, b"x && ".len())
}

/// `span_end`'s pair (resume, closed) for the word at `at`.
fn span_at(line: &[u8], at: usize) -> Option<(usize, bool)> {
    let tail = line.get(at..).unwrap_or(b"");
    let mut quotes = Quotes::new();
    span_end(line, tail, at, &mut quotes)
}

/// The word at `at` is mid-run (`x && <word>`), so `skip` sees `group_at_run`
/// == `Some(false)`; returns the resume, asserting it is a strict advance.
fn midrun(line: &[u8]) -> Option<usize> {
    let at = b"x && ".len();
    let resume = skip_at(line, at)?;
    assert!(
        resume > at,
        "resume {resume} must be past the word start {at}"
    );
    assert!(
        resume <= line.len(),
        "resume {resume} past the end of input"
    );
    Some(resume)
}

/// The three-way classification of the same command-position rule the group
/// opener uses: a run-start keyword is not this span (`block::open` owns it), a
/// keyword after `&&` / `||` is (skip whole), and a keyword-shaped word in an
/// argument position is word data (advance). The **fourth** arm — the word
/// behind a pipeline `|` — is not midrun's: it lives in `block::stage`
/// (task #292), so a `|` here is just an argument-position word.
#[test]
fn only_a_mid_run_opener_is_skipped() {
    // Run start: `Some(true)` — the separator path owns it.
    assert_eq!(skip_at(b"while :; do :; done", 0), None);
    assert_eq!(skip_at(b"if :; then :; fi", 0), None);
    // Cond part mid-run: `Some(false)`.
    assert_eq!(
        midrun(b"x && while :; do :; done"),
        Some(b"x && while :; do :; done".len())
    );
    assert_eq!(midrun(b"x && if :; then :; fi"), Some(21));
    // Argument position: `None` — word data; behind a `|` the stage arm takes
    // the span instead (task #292).
    assert_eq!(skip_at(b"echo while :; do :; done", 5), None);
    assert_eq!(skip_at(b"echo a | while :; do :; done", 8), None);
}

/// Every block-opening keyword is skipped mid-run: the run keeps the whole
/// block text and its own `;` never splits the run.
#[test]
fn every_opener_keyword_skips_its_span() {
    for tail in [
        b"if :; then echo t; fi".as_slice(),
        b"for i in 1; do echo $i; done".as_slice(),
        b"while :; do echo w; break; done".as_slice(),
        b"until :; do echo u; break; done".as_slice(),
        b"case x in x) echo c;; esac".as_slice(),
        b"wait\n readable %rd) echo r ;;\ndone".as_slice(),
    ] {
        let line = [b"x && "[..].to_vec(), tail[..].to_vec()].concat();
        assert_eq!(
            midrun(&line),
            Some(line.len()),
            "{line:?}: the whole run is one span"
        );
    }
}

/// The shared length rule: for every opener spelling `opener_len` measures
/// exactly the keyword whose keyword word `keyword_delta` scores `+1` — the
/// seed of the mid-run span cannot drift from the scored keyword (LESSONS:
/// sharing the scorer is not enough, the strip must be the same rule).
/// `span_end` takes the keyword branch only where that scorer says `+1`, so
/// the gate is the scorer itself and not a parallel rule.
/// `wait` is the gated opener: `opener_len` measures the word for both forms,
/// the score separates them — the event-case `wait` (a pattern keyword behind
/// it) takes the span, the POSIX builtin stays **word data**, because the cond
/// layer re-splits only `&&`/`||` and a swallowed `;` would drop the
/// statements after it (`true && wait; echo tail`).
#[test]
fn opener_len_agrees_with_the_scored_keyword() {
    for word in [
        b"case".as_slice(),
        b"if".as_slice(),
        b"for".as_slice(),
        b"while".as_slice(),
        b"until".as_slice(),
    ] {
        let len = opener_len(word).unwrap();
        assert_eq!(len, word.len(), "{word:?}");
        assert_eq!(
            keyword_delta(word, word, len),
            Some(1),
            "{word:?}: the seed and the score must agree"
        );
    }
    // `wait`: length 4 for both forms; `+1` only for the event-case form, and
    // the POSIX builtin is **no keyword at all** (task #309: the recognition
    // table has no `0` row — the old table's `Some(0)` reads the same at every
    // consumer, including this arm's `== Some(1)` gate).
    assert_eq!(opener_len(b"wait"), Some(4));
    assert_eq!(opener_len(b"wait $!"), Some(4));
    let block = b"wait\n readable %rd) x ;;\ndone";
    assert_eq!(keyword_delta(b"wait", block, 4), Some(1));
    assert_eq!(keyword_delta(b"wait", b"wait", 4), None);
    assert_eq!(keyword_delta(b"wait", b"wait $!", 4), None);
    // The mid-run span follows the score: the event-case form skips, the
    // builtin form is word data (`None`).
    let event = [b"x && "[..].to_vec(), block[..].to_vec()].concat();
    assert_eq!(midrun(&event).unwrap(), event.len());
    assert_eq!(midrun(b"x && wait $! && echo x"), None);
    // The boundary rule: glued forms are not keywords, closers and braces are
    // not openers, and the `time` prefix is measured off by `time_prefix_len`.
    for word in [
        b"whileish".as_slice(),
        b"iffy".as_slice(),
        b"fortran".as_slice(),
        b"casey".as_slice(),
        b"waiter".as_slice(),
        b"untilish".as_slice(),
        b"done".as_slice(),
        b"fi".as_slice(),
        b"esac".as_slice(),
        b"{".as_slice(),
        b"ls".as_slice(),
    ] {
        assert_eq!(opener_len(word), None, "{word:?}");
    }
}

/// The `keyword_delta` gate in `span_end`: a word the shared scorer scores `0`
/// opens no span, so a mid-run POSIX `wait` stays word data and the run keeps
/// its `;` split (the cond layer re-splits only `&&`/`||`: a span over the
/// builtin swallowed every statement after the first `;` and hung a loop whose
/// `break` sat in the swallowed text). The same word behind `time [-p]` is
/// gated on the stripped word, and a `wait` with a pattern keyword behind it
/// keeps its span.
#[test]
fn the_scored_gate_keeps_the_builtin_wait_word_data() {
    for tail in [
        b"wait".as_slice(),
        b"wait $!".as_slice(),
        b"wait 123".as_slice(),
        b"wait; echo tail".as_slice(),
        b"time wait".as_slice(),
        b"time -p wait".as_slice(),
        b"time time -p wait".as_slice(),
        b"wait \"readable\"".as_slice(),
        b"wait 123abc".as_slice(),
    ] {
        let line = [b"x && "[..].to_vec(), tail[..].to_vec()].concat();
        assert_eq!(midrun(&line), None, "{line:?}: a builtin opens no span");
    }
    // The event-case form keeps its span, bare and behind a `time` prefix.
    for prefix in [b"".as_slice(), b"time ".as_slice(), b"time -p ".as_slice()] {
        let line = [
            b"x && "[..].to_vec(),
            prefix[..].to_vec(),
            b"wait\n readable %rd) echo r ;;\ndone"[..].to_vec(),
        ]
        .concat();
        assert_eq!(midrun(&line).unwrap(), line.len(), "{line:?}");
    }
    // A `name()` word with no `{` body is not a span either: like the
    // run-start path (which requires `scan_function_block` to match), the
    // definition is only a block once its brace body is there.
    for tail in [
        b"f()".as_slice(),
        b"f()x".as_slice(),
        b"function g".as_slice(),
    ] {
        let line = [b"x && "[..].to_vec(), tail[..].to_vec()].concat();
        assert_eq!(midrun(&line), None, "{line:?}: no body, no span");
    }
}

/// A `time [-p]` prefix is transparent in the mid-run scorer too (LESSONS: a
/// statement-prefix keyword must be transparent in *every* first-word scorer):
/// the span is seeded on the keyword behind every prefix spelling, exactly like
/// the unprefixed form.
#[test]
fn time_prefixed_midrun_blocks_skip_the_same_span() {
    for tail in [
        b"while :; do echo hi; break; done".as_slice(),
        b"if :; then echo t; fi".as_slice(),
        b"for i in 1; do echo $i; done".as_slice(),
    ] {
        for prefix in [
            b"time ".as_slice(),
            b"time -p ".as_slice(),
            b"time time -p ".as_slice(),
        ] {
            let plain = [b"x && "[..].to_vec(), tail[..].to_vec()].concat();
            let mut line = b"x && ".to_vec();
            line.extend_from_slice(prefix);
            line.extend_from_slice(tail);
            let want = midrun(&plain).unwrap();
            let off = time_prefix_len(&line[b"x && ".len()..]).unwrap();
            assert_eq!(midrun(&line), Some(want + off), "{line:?}");
        }
    }
}

/// A `name() { … }` definition written after `&&` is a cond part too: the span
/// ends just past its closing brace, so the trailing `&& f` stays in the run.
#[test]
fn function_definition_midrun_skips_the_body_span() {
    let line = b"x && f() { echo F; } && f";
    // The span ends just past the closing brace; ` && f` is scanned after it.
    assert_eq!(midrun(line), Some(20));
    // The `function name { … }` spelling and a body with a nested block.
    assert_eq!(midrun(b"x && function g { if :; then :; fi; }"), Some(37));
    assert_eq!(midrun(b"x && g() { g() { :; }; }"), Some(24));
}

/// An unclosed span resumes at the end of input — the scan never stalls, and
/// the run flushes with the parse layer reporting the missing closer.
#[test]
fn unclosed_spans_resume_at_the_end_of_input() {
    for tail in [
        b"while".as_slice(),
        b"if".as_slice(),
        b"for".as_slice(),
        b"case".as_slice(),
        b"while :; do echo hi".as_slice(),
        b"if :; then echo t".as_slice(),
        b"f() { echo unclosed".as_slice(),
        b"while :; do echo \"unbalanced".as_slice(),
    ] {
        let line = [b"x && "[..].to_vec(), tail[..].to_vec()].concat();
        assert_eq!(midrun(&line), Some(line.len()), "{line:?}");
    }
}

/// The quote state is restored across the span (the `skip_group` rule): a
/// quoted `;` inside the block body stays the body's data, and the balanced
/// body leaves the scanner outside any quote.
#[test]
fn quote_state_is_restored_across_the_span() {
    let line = b"x && while :; do echo \"a;b\"; break; done";
    let mut book = Book::new();
    book.i = 5;
    assert_eq!(skip(line, &mut book), Some(line.len()));
    assert!(!book.state.in_quote, "the span's quotes must balance out");
    assert!(!book.state.in_single);
    assert!(!book.state.word_active, "the run continues at the resume");
    // A single-quoted body with the closer inside: the quote is opaque, so the
    // closer in it closes nothing — the scan consumes the span balanced, and
    // the state leaves the scanner outside the quote.
    let line = b"x && while :; do echo 'a; done'";
    let mut book = Book::new();
    book.i = 5;
    assert_eq!(skip(line, &mut book), Some(line.len()));
    assert!(!book.state.in_single, "the consumed span ends quote-free");
}

/// A `#` comment right after the closing keyword travels with the run: the
/// span ends at the comment's end (the closer's run ends at the end of
/// input), so the comment never becomes a statement.
#[test]
fn comment_after_the_closer_stays_in_the_run() {
    let line = b"x && while :; do :; done # comment";
    assert_eq!(midrun(line), Some(line.len() - 1));
}

/// A brace word is never this span: `{ …; }` is owned by `segment/group.rs`
/// (the scanner tests its opener first, and [`opener_len`] answers `None` for
/// a brace word), and the glued brace forms are plain words.
#[test]
fn brace_words_are_never_skipped_here() {
    assert_eq!(skip_at(b"x && { echo g; }", 4), None);
    assert_eq!(skip_at(b"x && {echo a;}", 4), None);
    assert_eq!(skip_at(b"x && {} ", 4), None);
}

/// `span_end` answers the pair (resume, closed): a closed span resumes at its
/// span end with `true`, an unclosed one resumes at the end of input with
/// `false` — the closedness the caller records for the REPL (task #294). A
/// word that opens no span answers `None` and records nothing.
#[test]
fn span_end_answers_the_resume_and_the_closedness() {
    for (tail, resume) in [
        (b"while :; do :; done".as_slice(), 24),
        (b"if :; then :; fi".as_slice(), 21),
        (b"f() { echo F; } && f".as_slice(), 20),
    ] {
        let line = [b"x && "[..].to_vec(), tail[..].to_vec()].concat();
        assert_eq!(span_at(&line, 5), Some((resume, true)), "{line:?}");
    }
    // The comment-behind-the-closer span is closed: it ends at the comment.
    let line = b"x && while :; do :; done # comment";
    assert_eq!(span_at(line, 5), Some((line.len() - 1, true)));
    for tail in [
        b"while".as_slice(),
        b"while :; do echo hi".as_slice(),
        b"f() { echo unclosed".as_slice(),
        b"while :; do echo \"unbalanced".as_slice(),
    ] {
        let line = [b"x && "[..].to_vec(), tail[..].to_vec()].concat();
        assert_eq!(span_at(&line, 5), Some((line.len(), false)), "{line:?}");
    }
    // The gated builtin opens no span at all.
    assert_eq!(span_at(b"x && wait; echo tail", 5), None);
}

/// The unclosed span opens the book's mid-run flag, the closed one leaves it
/// clear, and a word that opens no span (the gated builtin, a run-start
/// keyword, an argument-position keyword) leaves it clear too: the flag is set
/// by this arm and never by a shape that is not a span (task #294).
#[test]
fn the_unclosed_span_marks_the_book_open() {
    for tail in [
        b"while".as_slice(),
        b"if".as_slice(),
        b"for".as_slice(),
        b"case".as_slice(),
        b"while :; do echo hi".as_slice(),
        b"if :; then echo t".as_slice(),
        b"f() { echo unclosed".as_slice(),
        b"while :; do echo \"unbalanced".as_slice(),
    ] {
        let line = [b"x && "[..].to_vec(), tail[..].to_vec()].concat();
        assert!(flag(&line), "{line:?}: unclosed, so the REPL reads on");
    }
    for tail in [
        b"while :; do :; done".as_slice(),
        b"if :; then :; fi".as_slice(),
        b"case x in x) echo c;; esac".as_slice(),
        b"f() { echo F; } && f".as_slice(),
    ] {
        let line = [b"x && "[..].to_vec(), tail[..].to_vec()].concat();
        assert!(!flag(&line), "{line:?}: closed, so the line runs");
    }
    // Not a span at all: the builtin gate, a run-start keyword, an argument.
    assert!(!flag(b"x && wait; echo tail"));
    assert!(
        !flag_at(b"while :; do echo hi", 0),
        "the run-start arm owns it"
    );
    assert!(
        !flag_at(b"echo while :; do echo hi", 5),
        "an argument-position keyword is word data"
    );
}

/// The mid-run span takes the **same word-list seed** the run-start span is
/// seeded with (task #306, [`list_seed`]): a closer-shaped word of the `for`
/// list is word data, so the span is the whole block. `master` ended the span
/// at the list word, which left the run split in front of the block's own `do`.
///
/// The seed is `for`-only: an unclosed `for i in done` opens the book's mid-run
/// flag (the REPL's continuation signal), while a `while` whose *condition*
/// word is closer-shaped keeps the old span end — that word is an argument, and
/// the row is the seed's boundary as well as a mutant pin.
#[test]
fn a_for_list_word_never_ends_the_mid_run_span() {
    for tail in [
        b"for i in done; do :; done".as_slice(),
        b"for i in done; do echo $i; break; done".as_slice(),
    ] {
        let line = [b"x && "[..].to_vec(), tail[..].to_vec()].concat();
        assert_eq!(
            midrun(&line),
            Some(line.len()),
            "{line:?}: the whole run is one span"
        );
    }
    // The stage arm (`block::stage`) reuses this span verbatim (task #292), so
    // the pair is the block's own: closed, resuming at its last `done`.
    let line = b"for i in done; do :; done";
    assert_eq!(span_at(line, 0), Some((line.len(), true)));
    // The seed's boundary: `for` opens the flag when unclosed, `while` does not.
    assert!(flag(b"x && for i in done"));
    assert!(!flag(b"x && while done; do :; done"));
    assert!(!flag(b"x && case done in done) :;; esac"));
}
