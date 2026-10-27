//! The stray pattern paren of the clause an arm is missing its `;;` before
//! (task #300): bash reads an arm as a command list, and the `)` of the next
//! clause's pattern list is a token that list cannot take — so the missing
//! `;;` shows up as a `)` written in **command position** inside the arm.

use super::Token;
use crate::keywords::{Glue, Place, after, separator_data, word_place};
use crate::mask::{MaskBit, quoted_at};
use crate::parse::semi::glue_at;
use crate::parse::word_quoted::span_word;

/// Does `region` — one arm's tokens, `limit` the index of the `;;` that ends it
/// (`None`: the run up to `esac`) — carry the pattern paren of the clause that
/// follows it without a `;;`? `line` is the statement's text, for the
/// byte-level rows of [`after`].
///
/// The rule is the shared command-position machine ([`after`], LESSONS "One
/// clause rule, three scorers"), and the `)` counts only when it closes nothing
/// the arm opened. Each gate is measured against bash 5.3.9:
///
/// - **command position** ([`is_a_clause_pattern`]): `a) b) ;;`, `a) *) esac`,
///   `a) x=1) ;;` and `a) echo hi; b) ;;` (the `b` behind the `;`) bail, while a
///   paren behind an *argument* stays arm text (`a) echo hi) ;;`,
///   `a) echo a[b) ;;`, `a) (echo sub) ;;`) — task #419's family, left open here
///   so task #316's subshell-shaped arms keep parsing; a nested `case` arm's `)`
///   sits in [`Place::Pattern`] (behind `in`), so nested arms are untouched;
/// - **the parens the arm opened itself** ([`opens_groups`], counted as a
///   depth): the tokenizer emits every unquoted `)` as its own token, so a
///   definer's parameter list is the word `f(` plus a `)` token, a subshell the
///   arm wrote is a `(w` word plus a `)` token, and an array-literal assignment
///   is the word `m=(1` plus a `)` token — the group's `(` sits **mid-word**
///   there. bash parses all of these inside the arm — `a) f() { echo y; }; f ;;
///   esac`, `a) (cat) ;; esac`, `a) echo hi | (cat) ;; esac`,
///   `a) g() ( : ) ;; esac`, `a) m=(1) ;; esac`, `a) arr=("$@") ;; esac`, and
///   the definers/array literals an arm's `if`/`while`/`for`/`{ …; }` bodies
///   write behind a `;` are bash rc 0. A `)` at depth 0 is the only candidate
///   for a clause's pattern paren, so the rule reads the arm's own nesting, not
///   just the neighbouring word and not just the word's two edges;
/// - **the `((` region**: `for ((i=0;i<2;i++))` puts bare `)` tokens behind a
///   `;` — in command position — and inside the region every token is
///   arithmetic data; `((` is deliberately **not** an [`opens_groups`] paren,
///   wherever it sits — `m=$((1))` is a single word whose parens close inside it,
///   so counting them would hold the arm's depth above 0 and swallow the next
///   clause's paren (`a) m=$((1)); b) ;; esac` is bash rc 2) — and the region's
///   own rule owns them, so `a) ((1)); b) ;; esac` still bails;
/// - **an operator predecessor** ([`is_operator`]): `((1))` is the tokens `((1`,
///   `)`, `)` and its second `)` follows an operator token, not a pattern word;
/// - [`span_word`]: a quoted `")"` is a **word** whose text is `)` and whose
///   span carries the quote bytes, and an escaped `\)` keeps its pair in the
///   word text, so `a) echo ")" ;;` and `a) echo \) ;;` stay arm text. The span
///   gate is **stronger** here than the keyword gate [`keyword_word`], whose
///   scan-back breaks on the `)` byte itself (task #336);
/// - [`separator_data`]: a quoted `";"` inside the arm is word data, so it never
///   restarts command position for the `)` written behind it (task #336).
pub(crate) fn opens_next_pattern(region: &[Token], limit: Option<usize>, line: &[u8]) -> bool {
    // `place` is the place of the token being read, `before` the place of the
    // token before it — the word the `)`-test judges. The arm's first token
    // starts a command (the pattern's `)` is what precedes it), and [`after`]
    // needs the place of the word it reads, so the walk carries both.
    let mut place = Place::Command;
    let mut before = Place::Command;
    let mut arith = false;
    // The parens the **arm itself opened** and has not closed yet: `f(`, `(cat`,
    // the array literal `m=(1`. A `)` at depth 0 is the one that can be a
    // clause's pattern paren.
    let mut opened = 0usize;
    for (i, tok) in region
        .iter()
        .enumerate()
        .take(limit.unwrap_or(region.len()))
    {
        let (word, _start, end, _fq, mask, _spans) = tok;
        let Ok(bytes) = word.as_bytes() else {
            continue;
        };
        let paren = span_word(tok, b")");
        if arith {
            arith = !paren; // the arithmetic region ends at its own closing `)`
            continue;
        }
        // A `)` at depth 0 closes nothing the arm opened, so it is the one
        // candidate for a clause's pattern paren; any deeper `)` closes the
        // definer's `f(`, the arm's `(cat` group, its array literal `m=(1`.
        if paren && opened == 0 && is_a_clause_pattern(before, region, i) {
            return true;
        }
        opened = opened
            .saturating_sub(usize::from(paren))
            .saturating_add(opens_groups(bytes, mask));
        arith = bytes.starts_with(b"((");
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
            after(bytes, line, *end, place, glue, None)
        };
    }
    false
}

/// Is the `)` at index `i` the pattern paren of the clause that is missing its
/// `;;` before it? Its predecessor word must sit in command position (`b` in
/// `a) b) ;;`, behind the `;` in `a) echo hi; b) ;;`) and must not be an operator
/// token — a `)` written behind `;`, `|`, `&` or another `)` closes that operator,
/// never a pattern list. The operator test is the **token**'s, gated on the raw
/// span, so a quoted `";"` / `"|"` predecessor is word data and leaves the paren
/// behind it a clause paren (task #336).
fn is_a_clause_pattern(before: Place, region: &[Token], i: usize) -> bool {
    matches!(before, Place::Command)
        && i > 0
        && region.get(i - 1).is_some_and(|prev| !is_operator(prev))
}

/// How many groups this word opens **inside the arm**, counted over the word's
/// bytes with its quote mask, so the test sees a paren anywhere in the word and
/// ignores a paren that is word text:
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
/// - `(` **quoted** is word text: `echo "x(" b) ;;` opens nothing (task #419's
///   accepted shape, and the mask is what keeps it there), and a quoted `(` at the
///   arm's own start opens nothing either, so the `)` written behind that word is
///   the pattern paren (`a) "(x") ;;` is bash rc 2);
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
fn opens_groups(b: &[u8], mask: &[MaskBit]) -> usize {
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

/// The operator words the tokenizer emits as tokens of their own — every one of
/// them through [`span_word`], so a quoted spelling is a word, never an operator
/// predecessor (task #336).
fn is_operator(tok: &Token) -> bool {
    let w = |sep: &[u8]| span_word(tok, sep);
    w(b";") || w(b"|") || w(b"&") || w(b")")
}

#[cfg(test)]
mod tests;
