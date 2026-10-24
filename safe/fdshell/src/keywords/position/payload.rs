//! The **payload** half of [`super::after`]: what the place rule reads once the
//! glue's operator bytes are stripped from a word. A glued cond pair carries its
//! payload as the word's tail, so this pass is shared by the plain and the glued
//! rules: strip the pair's bytes, drop a `time [-p]` prefix, apply the rows that
//! are decided by the payload's shape alone (empty payload, data place, a
//! pipeline `|`), and hand the remaining word back to the keyword rows in
//! [`super::after`]. Every arm keeps its position in the sequence and its exact
//! byte arithmetic — the order *is* the rule.

use crate::keywords::time_prefix_len;

use super::{Glue, Place, glue};

/// Preprocess `w`'s payload at a [`Glue`] position: the pair's operator bytes,
/// the `time [-p]` prefix, and the payload-shape rules.
///
/// The result is `(decided, rest)`: `decided` is the [`Place`] a payload rule
/// reached (`open` for a payload-less glue, `Place::Data` behind a pair in data
/// position, the place of a pipeline stage), and `rest` is the payload the
/// keyword rows of [`super::after`] judge — so `None` means "fall through to
/// the rows", with `rest` already stripped of the glue bytes and the `time`
/// prefix.
pub(super) fn payload(
    w: &[u8],
    place: Place,
    glue: Glue,
    open: Place,
) -> (Option<Place>, &[u8]) {
    let n = glue::strip(glue);
    let rest = w.get(n..).unwrap_or(b"").trim_ascii();
    if rest.is_empty() {
        return (Some(open), rest);
    }
    // The glue pair's payload rows: behind data the pair's own word is an
    // argument (#292's carve-out: a block written as a pipeline stage), and an
    // operator run never starts a command. The payload has its operator bytes
    // stripped, so it reads the place the pair set.
    if matches!(glue, Glue::Head | Glue::Tail)
        && (matches!(place, Place::Data) || rest.starts_with(b"|"))
    {
        return (Some(Place::Data), rest);
    }
    let rest = time_prefix_len(rest)
        .and_then(|p| rest.get(p..))
        .unwrap_or(rest)
        .trim_ascii();
    if rest.is_empty() {
        // `time [-p]`: a statement prefix, not a word of its own.
        return (Some(place), rest);
    }
    // A word that *begins* with a pipeline `|` is the tail of a pipe byte, and
    // the stage behind a pipe is a command (task #292), exactly as the payload
    // of a cond pair is. Inside a `case` word list the `|` is the pattern
    // alternation (`case x in a|while)`), so the list stays a list; a `||`
    // spelled without its glue stays the argument row it always was.
    if rest.starts_with(b"|") && !rest.starts_with(b"||") {
        let stage = if matches!(place, Place::Pattern) {
            Place::Data
        } else {
            Place::Command
        };
        return (Some(stage), rest);
    }
    (None, rest)
}
