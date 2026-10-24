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
//! transition where it starts. The two flags are [`place`]; the payload
//! preprocessing they feed — the glue bytes, the `time [-p]` prefix, the rows a
//! payload's shape decides — is [`payload`], which hands the surviving word to
//! the rows of [`after`]. The payload's keyword rows live in
//! [`clause`], the one keyword set shared with the cond-boundary scan's
//! clause/body mode stack, so the two rules cannot drift apart. The two
//! **word-list** kinds are two places: a `case` list is [`Place::Pattern`],
//! a `for` list is [`Place::Words`], whose rule and gate live in [`list`]
//! (task #306).
//!
//! The place rule answers *where* a word sits; the **pairing** of the open
//! blocks (`crate::keywords::Pairing`, task #309) answers *which* block a closer
//! ends. Two orthogonal gates: [`after`] takes the innermost open block as `top`.

use crate::keywords::Closer;
use crate::segment::group::opens;

mod clause;
mod glue;
mod list;
mod payload;
mod place;
mod score;

pub(crate) use clause::{body_keyword, clause_keyword, list_keyword, list_opener};
pub(crate) use place::{Glue, Place};
pub(crate) use score::{closer_matches, scores};

/// The place of the word written after `word`: separators, cond operators, a
/// subshell `(`, a group opener and a `case` clause's glued `)` start a
/// command; a **closer** in command position ends the compound command it
/// closes and restarts command position (bash's grammar makes the `;` after a
/// compound command optional — the builder-side twin is
/// `parse/semi/preceded.rs::is_closer_predecessor`, task
/// #301); `if`/`while`/`until` (their conditions) and `then`/`else`/`elif`/`do`
/// (their bodies) put the word behind them in command position **only when they
/// are not an argument** (`echo a else fi` keeps `fi` an argument); `in` opens
/// a `case` word list ([`Place::Pattern`]) and a `for` in command position
/// opens its own **word list** ([`Place::Words`], task #306); a word that is
/// only a `time [-p]` prefix is transparent, keeping the current place; everything
/// else — an ordinary word, a closer-shaped argument — is an argument position,
/// so a keyword-shaped word there scores 0. A word that begins with a pipeline
/// `|` starts the stage behind it (task #292: a keyword block written as a
/// pipeline stage is a command), and an operator *form* (a redirect `&>`, whose
/// payload is operator bytes) is an argument.
///
/// `glue` marks the word's start in a cond pair ([`Glue`]): its payload is
/// read behind the pair's operator, from the place the pair sets — the
/// pair-form word `||if` puts `if` in command position (losing that let a
/// compound operand's closer close the enclosing block, task #236's S2), and
/// the bare tail byte (the tokenized `p||if`) is transparent, keeping the
/// place the pair set.
///
/// `top` is the closer the **innermost open block** requires
/// ([`crate::keywords::Pairing::top`], task #309): the closer row above is a
/// *pairing* row, so only that block's own closer restarts command position. A
/// caller with no block stack (the top level, `gate_end`'s start, a `case` arm's
/// stray-pattern walk) passes `None` and keeps the row as it always fired.
pub(crate) fn after(
    word: &[u8],
    line: &[u8],
    word_end: usize,
    place: Place,
    glue: Glue,
    top: Option<Closer>,
) -> Place {
    let w = word.trim_ascii();
    // A word of a `for` list keeps the list: only its separator or its `do`
    // leaves it ([`list::step`], task #306) — the scan must reach the block's
    // real closer, and a closer-shaped list word never closes the block.
    if matches!(place, Place::Words) {
        return list::step(w, glue);
    }
    // The place a *bare* glue (no payload) leaves: the head pair opens a
    // command, the tail byte is transparent, and a pipeline tail is a
    // separator whose place rule is #292's.
    let open = match glue {
        Glue::None => Place::Data,
        Glue::Head => Place::Command,
        Glue::Tail => place,
    };
    if matches!(glue, Glue::None) {
        if matches!(w, b"" | b";" | b"&&" | b"||" | b"&" | b"(" | b")") || w.ends_with(b")") {
            return Place::Command;
        }
        if opens(word, line, word_end) {
            return Place::Command;
        }
        // A closer in command position ends the compound command it closes, and
        // bash's grammar makes the `;` after a compound command optional, so the
        // word behind it is a command again (`while :; do if :; then :; fi done`,
        // `if :; then { :; } fi`). The gate is [`scores`]' own: a closer-shaped
        // *argument* (`echo fi`) leaves the place alone. Task #309 adds the
        // **kind** half ([`closer_matches`]): the closer must be the one `top`
        // requires, so a `fi` in a `while` body is an ordinary word and the
        // closer written behind it is an argument that scores nothing.
        if matches!(place, Place::Command) && closer_matches(w, line, word_end, top) {
            return Place::Command;
        }
    }
    // The payload pass — the glue bytes, the `time [-p]` prefix, and the rows
    // a payload's shape decides on its own — is [`payload`]; the keyword rows
    // below judge the word it leaves behind.
    let (decided, rest) = payload::payload(w, place, glue, open);
    if let Some(next) = decided {
        return next;
    }
    // The `)` is usually glued to its pattern (`x)`), so the arm's first
    // word is judged by the word's *last* byte; the command keywords open a
    // command, `in` opens a word list, every other payload is an argument.
    // A clause keyword opens a command **only** where it is not an argument:
    // `echo a else fi` writes `else` as `echo`'s word, so it must not put `fi`
    // in command position and swallow the closer (task #236's rule, read from
    // the transition side).
    let clause = !matches!(place, Place::Data);
    match rest.first() {
        Some(b'i') if clause && rest == b"if" => Place::Command,
        Some(b'i') if list_keyword(rest) => Place::Pattern,
        // A `for` in command position opens a **word list**: the words behind it
        // — its name, its `in`, its words — are list data ([`Place::Words`],
        // task #306). In a `case` pattern list `for` is a pattern word, so this
        // row is command-position-only and the `case` list keeps its own rule.
        Some(b'f') if matches!(place, Place::Command) && list_opener(rest) => Place::Words,
        Some(b'w') if clause && rest == b"while" => Place::Command,
        Some(b'u') if clause && rest == b"until" => Place::Command,
        // `then`/`else`/`do`/`elif`, one keyword set with the mode stack.
        _ if clause && (body_keyword(rest) || clause_keyword(rest)) => Place::Command,
        _ if rest.ends_with(b")") => Place::Command,
        _ => Place::Data,
    }
}

#[cfg(test)]
mod tests;
