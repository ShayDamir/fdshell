// mutants: -C --lib -C --test -C case -- -E 'test(/^parse::case_clause::/) | binary(case)'

//! The stray parens inside a `case` arm: bash reads an arm as a command list,
//! so the `)` of the next clause's pattern list is a token that list cannot
//! take when the `;;` before it is missing (task #300 — a `)` written in
//! **command position** inside the arm), and so is **any other depth-0 `)`**
//! the arm did not open itself (task #419 — behind an argument, behind an
//! operator token, or with no word before it at all: bash rc 2). This file is
//! the walk; the verdict it asks for lives in [`verdict`] (the classification,
//! the place gate and their tests).

use crate::keywords::{Glue, Place, after, separator_data, word_place};
use crate::mask::{MaskBit, quoted_at};
use crate::parse::case_clause::Token;
use crate::parse::semi::glue_at;
use crate::parse::word_quoted::span_word;
use verdict::{classify, opens_groups_at};

pub(crate) use verdict::{Stray, arith_seed};

/// Why the first `)` token of `region` — one arm's tokens, `limit` the index of
/// the `;;` that ends it (`None`: the run up to `esac`) — that the arm's command
/// list cannot take, as a [`Stray`] plus the span start of the judged token for
/// the position attachment (the mechanism `report_unbalanced_quote` uses; the
/// layer that detects owns the message). `line` is the statement's text, for
/// the byte-level rows of [`after`]. `arith_seed` is the closers a `((` word
/// **outside** the region — the clause's first pattern, read by the pattern
/// walk — still owns (task #419, and [`verdict`] for the region's own).
///
/// The rule is the shared command-position machine ([`after`], LESSONS "One
/// clause rule, three scorers"), and the `)` counts only when it closes nothing
/// the arm opened. Each gate is measured against bash 5.3.9:
///
/// - the classification of a judged paren ([`classify`]): task #300's command
///   position bails [`Stray::ClausePattern`]; a word-list place (`in` lists) is
///   the arm's own pattern paren and passes; everything else at depth 0 —
///   behind an argument (`a) echo hi) ;;`, `a) echo (b) ;;`), behind an operator
///   token (`a) echo hi; ) ;;`) or with no word before it at all (`a) ) ;;`) —
///   is the [`Stray::CloseParen`] family task #419 closed;
/// - **the place gate on the opener count** ([`opens_groups_at`]): an
///   argument-position word contributes no openers, so the `)` behind `echo (b`
///   is a depth-0 candidate — while the arm-started `a) (echo sub) ;;` (bash rc
///   0, task #316's shape) still closes the group the arm opened itself;
/// - **the parens the arm opened itself** ([`opens_groups`], counted as a
///   depth): the tokenizer emits every unquoted `)` as its own token, so a
///   definer's parameter list is the word `f(` plus a `)` token, a subshell the
///   arm wrote is a `(w` word plus a `)` token, and an array-literal assignment
///   is the word `m=(1` plus a `)` token — the group's `(` sits **mid-word**
///   there. bash parses all of these inside the arm — `a) f() { echo y; }; f ;;
///   esac`, `a) (cat) ;; esac`, `a) echo hi | (cat) ;; esac`,
///   `a) g() ( : ) ;; esac`, `a) m=(1) ;; esac`, `a) arr=("$@") ;; esac`, and
///   the definers/array literals an arm's `if`/`while`/`for`/`{ …; }` bodies
///   write behind a `;` are bash rc 0. A `)` at depth 0 is the only candidate,
///   so the rule reads the arm's own nesting, not just the neighbouring word
///   and not just the word's two edges;
/// - **the `((` region**, now its own pair of closers: bash's arithmetic
///   command is `(( … ))`, so a word whose bytes start with `((` arms **two**
///   `)` tokens of arithmetic data — `a) ((1)) ;; esac` is bash rc 0, each
///   closer consumed in order and never judged, and every other token inside
///   the region stays data (`for ((i=0;i<2;i++))` writes bare `)` tokens behind
///   its `;`, in command position). `((` is deliberately **not** an
///   [`opens_groups`] paren, wherever it sits — `m=$((1))` is a single word
///   whose parens close inside it, so counting them would hold the arm's depth
///   above 0 and swallow the next clause's paren (`a) m=$((1)); b) ;; esac` is
///   bash rc 2) — and the region's own rule owns them, so `a) ((1)); b) ;; esac`
///   still bails;
/// - [`span_word`]: a quoted `")"` is a **word** whose text is `)` and whose
///   span carries the quote bytes, and an escaped `\)` keeps its pair in the
///   word text, so `a) echo ")" ;;` and `a) echo \) ;;` stay arm text. The span
///   gate is **stronger** here than the keyword gate [`keyword_word`], whose
///   scan-back breaks on the `)` byte itself (task #336);
/// - [`separator_data`]: a quoted `";"` inside the arm is word data, so it never
///   restarts command position for the `)` written behind it (task #336 — the
///   paren lands in **argument** position and the argument arm of the verdict
///   takes it; the **unquoted** twins stay [`Stray::ClausePattern`]).
pub(crate) fn verdict(
    region: &[Token],
    limit: Option<usize>,
    line: &[u8],
    arith_seed: usize,
) -> Option<(Stray, usize)> {
    // `place` is the place of the token being read, `before` the place of the
    // token before it — the word the `)`-test judges. The arm's first token
    // starts a command (the pattern's `)` is what precedes it), and [`after`]
    // needs the place of the word it reads, so the walk carries both.
    let mut place = Place::Command;
    let mut before = Place::Command;
    // The closers the `((` region still owns (task #419): bash's arithmetic
    // command is `(( … ))`, so the `((` word arms two `)` tokens of data.
    let mut arith_closers = arith_seed;
    // The parens the **arm itself opened** and has not closed yet: `f(`, `(cat`,
    // the array literal `m=(1`. A `)` at depth 0 is the only candidate.
    let mut opened = 0usize;
    for (i, tok) in region
        .iter()
        .enumerate()
        .take(limit.unwrap_or(region.len()))
    {
        let Token {
            word,
            start,
            end,
            mask,
            ..
        } = tok;
        let Ok(bytes) = word.as_bytes() else {
            continue;
        };
        let paren = span_word(tok, b")");
        if arith_closers > 0 {
            // Inside the region every token is arithmetic data; each closer
            // consumed brings the region to its own end.
            arith_closers -= usize::from(paren);
            continue;
        }
        // A `)` at depth 0 closes nothing the arm opened, so it is the one
        // candidate for a verdict; any deeper `)` closes the definer's `f(`,
        // the arm's `(cat` group, its array literal `m=(1`.
        if paren
            && opened == 0
            && let Some(stray) = classify(before, region, i)
        {
            return Some((stray, *start));
        }
        opened = opened
            .saturating_sub(usize::from(paren))
            .saturating_add(opens_groups_at(place, bytes, mask));
        arith_closers = usize::from(bytes.starts_with(b"((")) * 2;
        // Index 0 has no predecessor token: [`glue_at`] reads `tokens[i - 1]`.
        let glue = if i == 0 {
            Glue::None
        } else {
            glue_at(region, i, bytes)
        };
        before = place;
        // No block stack here: this walk reads a `case` arm's stray-pattern
        // parens, and the arm-level opener/closer pairing is the `case`
        // builder's (it walks with `Closer::Esac`), so the closer row of
        // [`after`] is asked with `None` — the top-level behavior (task #309).
        // A quoted `";"` is word data, so it leaves the place an ordinary word
        // leaves and never reopens command position for the `)` behind it
        // (task #336).
        place = if separator_data(tok) {
            word_place(place)
        } else {
            after(bytes, line, *end, place, glue, None, mask)
        };
    }
    None
}

/// How many groups this word opens **inside the arm**, counted over the word's
/// bytes with its quote mask, so the test sees a paren anywhere in the word and
/// ignores a paren that is word text. Only asked in command position
/// ([`opens_groups_at`] — the place gate):
///
/// - a definer's parameter list ends with the paren (`f(`, whose `)` ends `f()`
///   and whose body is the arm's command), a subshell word opens one (`(cat`,
///   `(echo`, a bare `(`: `a) (cat) ;; esac`, `a) g() ( : ) ;; esac`,
///   `a) echo hi | (cat) ;; esac` are bash rc 0);
/// - an **array-literal assignment** carries the paren mid-word: the tokenizer's
///   word is `m=(1` (mask: every byte unquoted) and the array's `)` is a token of
///   its own, so a test on the word's two edges misses the group and the array's
///   `)` reads as the next clause's pattern paren. `case x in a) m=(1) ;; esac`,
///   `a) x=(1) y=2 ;;`, `a) echo hi; x=(1) ;;`, `a) arr=("$@") ;;`,
///   `a) arr=("a") ;;`, `a) m=() ;;` are bash rc 0, and so are the array
///   literals an arm's `if`/`while`/`{ …; }` bodies write behind a `;`;
/// - `(` **quoted** is word text: `echo "x(" b) ;;` opens nothing, and the
///   `)` written behind them is the arm's stray paren (task #419, bash rc 2);
///   a quoted `(` at the arm's own start opens nothing either, so the `)`
///   written behind that word is the pattern paren (`a) "(x") ;;` is bash rc 2);
/// - an **escape pair** `\X` is one literal word byte — the token's mask marks
///   both bytes `Unquoted`, so the backslash is the only marker — and its second
///   byte never opens a group: `a) echo x\() b) ;; esac` is bash rc 2, and the
///   `x\(` word must not absorb the clause paren that follows it. Only an
///   **unquoted** backslash consumes the byte after it: in `"\\"(x)` the `(`
///   behind the quoted pair opens its group;
/// - `((` is **not** a group opener, wherever it sits — the arithmetic region's
///   own rule owns those parens, and counting them would blind the scan to the
///   clause paren that follows (`a) ((1)); b) ;; esac` is bash rc 2).
///   `(`/`<`/`>` are never operator tokens on their own
///   (`parse/token/step.rs`), so here they are word bytes.
pub(super) fn opens_groups(b: &[u8], mask: &[MaskBit]) -> usize {
    let mut opened = 0usize;
    let mut i = 0;
    while let Some(&byte) = b.get(i) {
        match byte {
            // An escape pair is one literal word byte: skip the escaped byte too.
            b'\\' if !quoted_at(mask, i) => i += 2,
            // A quoted paren is word text.
            b'(' if quoted_at(mask, i) => i += 1,
            // `((` is the arithmetic region's own pair: that rule owns the parens.
            b'(' if b.get(i + 1) == Some(&b'(') => i += 2,
            b'(' => {
                opened += 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    opened
}

mod verdict;

#[cfg(test)]
mod tests;
