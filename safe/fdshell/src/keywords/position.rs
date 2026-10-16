//! Command position of a word in a keyword run (task #236): bash recognizes a
//! reserved word in command position *only*, so a keyword-shaped *argument*
//! written inside a block body (`echo if && echo x`, `echo done`) is word
//! data, never a block keyword. The top-level half of this rule is the segment
//! scanner's [`group_at_run`]; the in-block half needs the clause keywords
//! (`then`/`do`/`else`/`elif`), the `case` clause `)` and the `case` word list,
//! which a run-start test cannot see — so the rule is a state machine over the
//! words, carried as a [`Place`] flag by every scorer that walks block text
//! (`cond/boundary/block`, `comment/depth_delta`, `parse/semi/closing`).

use crate::keywords::{keyword_delta, time_prefix_len};
use crate::segment::group::{closes, opens};

/// Where the word written next sits, as far as the keyword scorers can tell.
/// The state transitions are [`after`]; the gate is [`scores`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    /// A command may start here: the run's start, behind a cond operator,
    /// after `then`/`do`/`else`/`elif`, after a condition-taking opener, a
    /// group opener or a `case` clause's `)`. Only here does a block keyword
    /// score.
    Command,
    /// A `case` word list (`case x in …`). Its patterns are words, so no
    /// opener scores there, and `esac` is the list's terminator, so a closer
    /// does (`case x in esac` is a complete `case`).
    Pattern,
    /// An argument position: every word here is data.
    Data,
}

/// May the keyword word be scored at `place`? An opener scores in command
/// position only; a closer in command position *or* at a `case` list — bash
/// requires a separator before a closing keyword (`done echo x` is a syntax
/// error), so a closer in argument position is word data.
///
/// A brace word is never gated: a literal `{`/`}` argument eating the block's
/// closer is the accepted divergence
/// `tests/group.rs::literal_brace_argument_in_a_block_body` pins; its
/// clause-position rule belongs to tasks #189/#193.
pub(crate) fn scores(word: &[u8], line: &[u8], word_end: usize, place: Place) -> bool {
    if opens(word, line, word_end) || closes(word) {
        return true;
    }
    match keyword_delta(word, line, word_end) {
        Some(delta) if delta > 0 => place == Place::Command,
        Some(delta) if delta < 0 => matches!(place, Place::Command | Place::Pattern),
        _ => false,
    }
}

/// The place of the word written after `word`: separators, cond operators, a
/// subshell `(`, a group opener and a `case` clause's glued `)` start a
/// command; `if`/`while`/`until` (their conditions), `then`/`else`/`elif`/`do`
/// (their bodies) put the word behind them in command position; `in` opens a
/// `case`/`for` word list ([`Place::Pattern`]); a word that is only a
/// `time [-p]` prefix is transparent, keeping the current place; everything
/// else — an ordinary word, a closer, a pipeline `|` (the accepted divergence
/// of a block written as a pipeline stage, task #292) — is an argument
/// position, so a keyword-shaped word there scores 0.
pub(crate) fn after(word: &[u8], line: &[u8], word_end: usize, place: Place) -> Place {
    let w = word.trim_ascii();
    // The `)` is usually glued to its pattern (`x)`), so the arm's first word
    // is judged by the word's *last* byte.
    if matches!(w, b"" | b";" | b"&&" | b"||" | b"&" | b"(" | b")") || w.ends_with(b")") {
        return Place::Command;
    }
    if opens(word, line, word_end) {
        return Place::Command;
    }
    let rest = time_prefix_len(w)
        .and_then(|p| w.get(p..))
        .unwrap_or(w)
        .trim_ascii();
    if rest.is_empty() {
        // `time [-p]`: a statement prefix, not a word of its own.
        return place;
    }
    match rest {
        b"if" | b"while" | b"until" | b"then" | b"else" | b"elif" | b"do" => Place::Command,
        b"in" => Place::Pattern,
        _ => Place::Data,
    }
}

#[cfg(test)]
mod tests;
