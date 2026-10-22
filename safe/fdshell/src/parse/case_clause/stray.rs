//! The stray pattern paren of the clause an arm is missing its `;;` before
//! (task #300): bash reads an arm as a command list, and the `)` of the next
//! clause's pattern list is a token that list cannot take — so the missing
//! `;;` shows up as a `)` written in **command position** inside the arm.

use super::Token;
use crate::keywords::{Glue, Place, after};
use crate::parse::semi::glue_at;
use crate::parse::word_quoted::keyword_word;

/// Does `region` — one arm's tokens, `limit` the index of the `;;` that ends it
/// (`None`: the run up to `esac`) — carry the pattern paren of the clause that
/// follows it without a `;;`? `line` is the statement's text, for the keyword
/// gate [`keyword_word`] reads.
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
/// - **the parens the arm opened itself** ([`opens_a_group`], counted as a
///   depth): the tokenizer emits every unquoted `)` as its own token, so a
///   definer's parameter list is the word `f(` plus a `)` token and a subshell
///   the arm wrote is a `(w` word plus a `)` token. bash parses both inside the
///   arm — `a) f() { echo y; }; f ;; esac`, `a) (cat) ;; esac`,
///   `a) echo hi | (cat) ;; esac`, `a) g() ( : ) ;; esac`, and the definers an
///   arm's `if`/`while`/`for`/`{ …; }` bodies write behind a `;` are bash rc 0.
///   A `)` at depth 0 is the only candidate for a clause's pattern paren, so the
///   rule reads the arm's own nesting, not just the neighbouring word;
/// - **the `((` region**: `for ((i=0;i<2;i++))` puts bare `)` tokens behind a
///   `;` — in command position — and inside the region every token is
///   arithmetic data; `((` is deliberately not an [`opens_a_group`] paren, so
///   the region's own rule owns it and `a) ((1)); b) ;; esac` still bails;
/// - **an operator predecessor** ([`is_operator`]): `((1))` is the tokens `((1`,
///   `)`, `)` and its second `)` follows an operator token, not a pattern word;
/// - [`keyword_word`]: a quoted `")"` is a word whose text is `)`, and an
///   escaped `\)` keeps its pair in the word text, so `a) echo ")" ;;` stays
///   arm text (the same gate the keyword scorers open with).
pub(crate) fn opens_next_pattern(region: &[Token], limit: Option<usize>, line: &[u8]) -> bool {
    // `place` is the place of the token being read, `before` the place of the
    // token before it — the word the `)`-test judges. The arm's first token
    // starts a command (the pattern's `)` is what precedes it), and [`after`]
    // needs the place of the word it reads, so the walk carries both.
    let mut place = Place::Command;
    let mut before = Place::Command;
    let mut arith = false;
    // The parens the **arm itself opened** and has not closed yet: `f(`, `(cat`.
    // A `)` at depth 0 is the one that can be a clause's pattern paren.
    let mut opened = 0usize;
    for (i, tok) in region
        .iter()
        .enumerate()
        .take(limit.unwrap_or(region.len()))
    {
        let (word, _start, end, _fq, _mask, _spans) = tok;
        let Ok(bytes) = word.as_bytes() else {
            continue;
        };
        let paren = keyword_word(tok, line, b")");
        if arith {
            arith = !paren; // the arithmetic region ends at its own closing `)`
            continue;
        }
        if paren {
            if let Some(rest) = opened.checked_sub(1) {
                opened = rest; // closes the definer's `f(`, the arm's own `(` group
            } else if is_a_clause_pattern(before, region, i) {
                return true;
            }
        }
        arith = bytes.starts_with(b"((");
        if opens_a_group(bytes) {
            opened += 1;
        }
        // Index 0 has no predecessor token: [`glue_at`] reads `tokens[i - 1]`.
        let glue = if i == 0 {
            Glue::None
        } else {
            glue_at(region, i, bytes)
        };
        before = place;
        place = after(bytes, line, *end, place, glue);
    }
    false
}

/// Is the `)` at index `i` the pattern paren of the clause that is missing its
/// `;;` before it? Its predecessor word must sit in command position (`b` in
/// `a) b) ;;`, behind the `;` in `a) echo hi; b) ;;`) and must not be an operator
/// token — a `)` written behind `;`, `|`, `&` or another `)` closes that operator,
/// never a pattern list.
fn is_a_clause_pattern(before: Place, region: &[Token], i: usize) -> bool {
    matches!(before, Place::Command)
        && i > 0
        && region
            .get(i - 1)
            .is_some_and(|prev| !prev.0.as_bytes().is_ok_and(is_operator))
}

/// Does this word **open** a group the arm owns, so that a later `)` closes it
/// and is arm text? A definer's parameter list ends with the paren (`f(`, whose
/// `)` ends `f()` and whose body is the arm's command), a subshell word opens one
/// (`(cat`, `(echo`, a bare `(`: `a) (cat) ;; esac`, `a) g() ( : ) ;; esac`,
/// `a) echo hi | (cat) ;; esac` are bash rc 0). The arithmetic `((` is **not**
/// such a group — its own region rule owns those parens, and counting it here
/// would blind the scan to the clause paren that follows (`a) ((1)); b) ;; esac`
/// is bash rc 2). `(`/`<`/`>` are never operator tokens on their own
/// (`parse/token/step.rs`), so here they are word bytes.
fn opens_a_group(b: &[u8]) -> bool {
    (b.starts_with(b"(") && !b.starts_with(b"((")) || b.ends_with(b"(")
}

/// The operator words the tokenizer emits as tokens of their own.
fn is_operator(b: &[u8]) -> bool {
    matches!(b, b";" | b"|" | b"&" | b")")
}

#[cfg(test)]
mod tests;
