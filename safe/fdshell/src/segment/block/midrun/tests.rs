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
/// argument position is word data (advance).
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
    // Argument position: `None` — word data (`|` is one too, task #292's gap).
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
/// `wait` is the gated opener: the length is measured on the word, the block
/// score needs the pattern keyword behind it, and the mid-run scan closes at
/// the `done` it finds — a `wait` that opens no block swallows nothing the
/// cond layer does not re-split.
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
    // `wait`: length 4; `+1` only for the event-case form (the POSIX builtin
    // scores `0`, and the builtin mid-run keeps its operators through the
    // cond re-split).
    assert_eq!(opener_len(b"wait"), Some(4));
    let block = b"wait\n readable %rd) x ;;\ndone";
    assert_eq!(keyword_delta(b"wait", block, 4), Some(1));
    assert_eq!(keyword_delta(b"wait", b"wait", 4), Some(0));
    assert_eq!(midrun(b"x && wait $! && echo x"), Some(22));
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
