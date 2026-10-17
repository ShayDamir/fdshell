//! Command position of a word in a keyword run (task #236): bash recognizes a
//! reserved word in command position *only*, so a keyword-shaped *argument*
//! written inside a block body (`echo if && echo x`, `echo done`) is word
//! data, never a block keyword. The top-level half of this rule is the segment
//! scanner's [`group_at_run`]; the in-block half needs the clause keywords
//! (`then`/`do`/`else`/`elif`), the `case` clause `)` and the `case` word list,
//! which a run-start test cannot see — so the rule is a state machine over the
//! words, carried as a [`Place`] flag by every scorer that walks block text
//! (`cond/boundary/block`, `comment/depth_delta`, `parse/semi/closing`).
//! A glued cond pair is *two* word starts — its head and its tail byte — and
//! a word can begin at either, so a word carries a [`Glue`] telling the
//! transition where it starts.

use crate::keywords::{keyword_delta, time_prefix_len};
use crate::segment::group::{closes, opens};

mod glue;

/// Where the word written next sits, as far as the keyword scorers can tell.
/// The state transitions are [`after`]; the gate is [`scores`].
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
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

/// Where a word starts inside a glued `&&`/`||` cond pair (task #236's S2).
/// The pair is *two* word starts: [`is_word_break`](crate::scan::advance)
/// breaks words at operator bytes, so `p||if` is scanned as the words `p`,
/// `||if` (the pair's head) and `|if` (its tail), and the tokenized `p||if`
/// carries the bare halves as separate tokens. The tail byte of a *pipeline*
/// `|` is a single-byte run — [`Glue::None`], #292's carve-out.
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(crate) enum Glue {
    /// Outside a cond pair.
    None,
    /// At the pair's first byte: the payload after the pair is in command
    /// position.
    Head,
    /// At a later byte of the pair: the word continues the pair's command
    /// position (bare tail: transparent, see [`after`]).
    Tail,
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
        Some(delta) if delta > 0 => matches!(place, Place::Command),
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
/// else — an ordinary word, a closer — is an argument position, so a
/// keyword-shaped word there scores 0. A pipeline tail behind data stays data
/// (the accepted divergence of a block written as a pipeline stage, task
/// #292) and an operator *form* (a redirect `&>`, whose payload is operator
/// bytes) is an argument.
///
/// `glue` marks the word's start in a cond pair ([`Glue`]): its payload is
/// read behind the pair's operator, from the place the pair sets — the
/// pair-form word `||if` puts `if` in command position (losing that let a
/// compound operand's closer close the enclosing block, task #236's S2), and
/// the bare tail byte (the tokenized `p||if`) is transparent, keeping the
/// place the pair set.
pub(crate) fn after(word: &[u8], line: &[u8], word_end: usize, place: Place, glue: Glue) -> Place {
    let w = word.trim_ascii();
    // The place a *bare* glue (no payload) leaves: the head pair opens a
    // command, the tail byte is transparent, and a pipeline tail is a
    // separator whose place rule is #292's.
    let open = match glue {
        Glue::None => Place::Data,
        Glue::Head => Place::Command,
        Glue::Tail => place,
    };
    let n = glue::strip(glue);
    let pipeline = match glue {
        Glue::None => {
            if matches!(w, b"" | b";" | b"&&" | b"||" | b"&" | b"(" | b")") || w.ends_with(b")") {
                return Place::Command;
            }
            if opens(word, line, word_end) {
                return Place::Command;
            }
            glue::tail(w).is_some()
        }
        Glue::Head | Glue::Tail => true,
    };
    let rest = w.get(n..).unwrap_or(b"").trim_ascii();
    if rest.is_empty() {
        return open;
    }
    if rest.iter().all(|&b| b == b'&' || b == b'|') {
        return Place::Data;
    }
    // The pipeline tail rows: behind data an argument (#292's carve-out),
    // and the `|`-starting separator word in any place. A glue payload has
    // its operator bytes stripped, so it reads the pair's command position.
    if pipeline && (place == Place::Data || rest.starts_with(b"|")) {
        return Place::Data;
    }
    let rest = time_prefix_len(rest)
        .and_then(|p| rest.get(p..))
        .unwrap_or(rest)
        .trim_ascii();
    if rest.is_empty() {
        // `time [-p]`: a statement prefix, not a word of its own.
        return place;
    }
    // The `)` is usually glued to its pattern (`x)`), so the arm's first
    // word is judged by the word's *last* byte; the command keywords open a
    // command, `in` opens a word list, every other payload is an argument.
    match rest.first() {
        Some(b'i') if rest == b"if" => Place::Command,
        Some(b'i') => Place::Pattern,
        Some(b'w') if rest == b"while" => Place::Command,
        Some(b'u') if rest == b"until" => Place::Command,
        Some(b't') if rest == b"then" => Place::Command,
        Some(b'e') if rest == b"else" || rest == b"elif" => Place::Command,
        Some(b'd') if rest == b"do" => Place::Command,
        _ if rest.ends_with(b")") => Place::Command,
        _ => Place::Data,
    }
}

#[cfg(test)]
mod tests;
