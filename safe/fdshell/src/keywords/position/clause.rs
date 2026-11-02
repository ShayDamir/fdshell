//! The clause keywords of a keyword run, as one keyword set shared by two
//! rules that must stay in step: [`super::after`]'s command-position
//! transitions and the cond-boundary scan's clause/body mode stack
//! (`cond/boundary/block/clause`). Each predicate is one row of [`super::after`]'s
//! payload match, so a keyword added to the grammar lands in both rules at once.

use crate::bytes::word_break;

/// Does `w` put the words behind it into a block **body** — `then` (with its
/// `if`/`elif`), `else`, `do`?
pub(crate) fn body_keyword(w: &[u8]) -> bool {
    matches!(w, b"then" | b"else" | b"do")
}

/// Does `w` put the words behind it back into a block **clause** — `elif`
/// starts a fresh condition?
pub(crate) fn clause_keyword(w: &[u8]) -> bool {
    w == b"elif"
}

/// Does `w` open a `case`/`for` **word list** — `in`?
pub(crate) fn list_keyword(w: &[u8]) -> bool {
    w == b"in"
}

/// Does `w` open a block that **owns a word list** — `for`? Its list is the
/// [`super::list`] rule's [`super::Place::Words`] place, which the scan seeds
/// from the opener (a block's own list, whose word lies before the scan) and
/// reaches through this row (a nested block's list). The row reads the one
/// boundary rule ([`crate::bytes::word_break`], task #396), so the arithmetic
/// `for ((…))` spelled glued — `for((…))`, whose `(` ends the keyword word at
/// both granularities — **does** open the list place.
pub(crate) fn list_opener(w: &[u8]) -> bool {
    w.starts_with(b"for") && w.get(b"for".len()).is_none_or(|&b| word_break(b))
}
