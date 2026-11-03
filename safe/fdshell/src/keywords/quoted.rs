//! The quoted-word rule of keyword recognition (task #141).
//!
//! One rule, seen from the span and from the keyword: [`quoted_word`] measures a
//! word's own raw span, and [`keyword_is_quoted`] is keyword recognition's gate — it
//! measures the keyword's **own** bytes where they lie in `line` ([`span_start`]), so
//! the bytes behind the byte that breaks the word belong to what is glued to it (bash
//! splits `done>f` at the `>`).

use crate::bytes::{QUOTE, SQUOTE, word_break};

/// A word written with quote bytes is data, never a keyword: `"fi"`, `'done'`,
/// `"{"` are words (POSIX §2.9.1 recognizes only unquoted keyword tokens, and
/// bash treats a quoted keyword as a command word). The word's raw span is the
/// run ending at `word_end`, back to the byte that breaks a word.
///
/// The one **span** gate at both granularities: the parse layer calls it through
/// `parse::word_quoted::keyword_word` (task #247) and the definer through
/// `keywords::function`, so the block-depth scan and the block builders cannot
/// disagree. Keyword recognition calls [`keyword_is_quoted`], which reaches it through
/// [`span_start`] — on the keyword's own bytes when the word is glued to an operand.
pub(crate) fn quoted_word(line: &[u8], word_end: usize) -> bool {
    let mut i = word_end;
    let mut quoted = false;
    while i > 0 {
        match line.get(i - 1).copied() {
            Some(b) if word_break(b) => break,
            Some(b) => quoted |= b == QUOTE || b == SQUOTE,
            None => break,
        }
        i -= 1;
    }
    quoted
}

/// The gate of the recognition table [`super::kind`] (task #413): the word's **own**
/// bytes decide, and they are its first `kw_len` — the keyword prefix the row matched
/// — **where they lie in `line`** ([`span_start`]). A word glued to an operand —
/// `done>f"f"`, `fi>f`, `for(("$x")`, `a&&if>x"y"` — **is** the keyword: what sits
/// behind the breaking byte is the operand's, and bash splits the word there. The
/// byte-level readers hand `word` as the raw span, so `word.len()` locates it and the
/// measurement is the offset `word_end − (word.len() − kw_len)`. The token-level
/// reader (`parse::semi::closing`) hands **quote-removed text**, whose length is not
/// the span's, and the offset would land inside the operand's quotes: read from the
/// span start, a fully quoted word whose text carries a break byte (`"done x"`,
/// `"fi x"`, `"fi>y"`) has its own bytes inside a quote and stays data (POSIX §2.9.1,
/// bash's answer), while `case>x"y"`-style glue stays the keyword. A `word` that
/// matches `line` nowhere is a hand-written pin whose `word`/`word_end` pair is
/// inconsistent (`keywords/tests.rs::time_prefix_is_transparent_for_the_depth_rule`,
/// pinned at `keywords/kind/tests.rs::the_gate_of_an_unalignable_word_is_the_row`):
/// the row matched the keyword's literal prefix, so the word is the keyword.
pub(crate) fn keyword_is_quoted(word: &[u8], line: &[u8], word_end: usize, kw_len: usize) -> bool {
    match span_start(word, line, word_end) {
        Some(start) => quoted_word(line, start + kw_len),
        None => false,
    }
}

/// Where `word`'s bytes begin in `line`: walk back from `word_end` matching the word,
/// skipping the quote bytes the tokenizer removed from its span (the only way the
/// token-level reader's text differs from the line's bytes). `None` when the bytes do
/// not line up — the pins whose `word`/`word_end` pair is inconsistent.
fn span_start(word: &[u8], line: &[u8], word_end: usize) -> Option<usize> {
    let (mut w, mut i) = (word.len(), word_end);
    while w > 0 {
        let (Some(&wb), Some(&lb)) = (word.get(w - 1), i.checked_sub(1).and_then(|j| line.get(j)))
        else {
            return None;
        };
        if lb == wb {
            w -= 1;
            i -= 1;
        } else if lb == QUOTE || lb == SQUOTE {
            i -= 1;
        } else {
            return None;
        }
    }
    Some(i)
}
