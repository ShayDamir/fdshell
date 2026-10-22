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
/// clause rule, three scorers"), so the `)` counts only when the word right
/// before it **sits** in command position and that word is a word, not an
/// operator token. Each gate is measured against bash 5.3.9:
///
/// - **command position**: `a) b) ;;`, `a) *) esac`, `a) x=1) ;;` and
///   `a) echo hi; b) ;;` (the `b` behind the `;`) bail, while a paren behind an
///   *argument* stays arm text (`a) echo hi) ;;`, `a) echo a[b) ;;`,
///   `a) (echo sub) ;;`) — task #419's family, left open here so task #316's
///   subshell-shaped arms keep parsing; a nested `case` arm's `)` sits in
///   [`Place::Pattern`] (behind `in`), so nested arms are untouched;
/// - **word predecessor**: the tokenizer emits every unquoted `)` as its own
///   token, so `((1))` is the tokens `((1`, `)`, `)` and its second `)` follows
///   an operator: it closes the arithmetic command, not a pattern list;
/// - **the `((` region**: `for ((i=0;i<2;i++))` puts bare `)` tokens behind a
///   `;` — in command position — and inside the region every token is
///   arithmetic data;
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
        // `i > 0`: a region that starts at the paren has no word before it.
        if paren
            && matches!(before, Place::Command)
            && i > 0
            && region.get(i - 1).is_some_and(|prev| !is_operator(prev))
        {
            return true;
        }
        arith = bytes.starts_with(b"((");
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

/// The operator words the tokenizer emits as tokens of their own: a `)` behind
/// one of them closes that operator (the `;;` pair, `((1))`), never a clause's
/// pattern list. `(`/`<`/`>` are never separate tokens (`parse/token/step.rs`),
/// so here they are word bytes.
fn is_operator(tok: &Token) -> bool {
    tok.0
        .as_bytes()
        .is_ok_and(|b| matches!(b, b";" | b"|" | b"&" | b")"))
}

#[cfg(test)]
mod tests;
