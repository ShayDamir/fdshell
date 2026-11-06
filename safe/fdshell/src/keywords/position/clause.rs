//! The clause keywords of a keyword run, as one keyword set shared by two
//! rules that must stay in step: [`super::after`]'s command-position
//! transitions and the cond-boundary scan's clause/body mode stack
//! (`cond/boundary/block/clause`). Each predicate is one row of [`super::after`]'s
//! payload match, so a keyword added to the grammar lands in both rules at once.
//! The one exception is [`arm_clause_block`], the mode stack's own `)` row: it
//! answers which *kind* of block the `)`-ending word belongs to, and
//! [`super::after`]'s `)` row stays ungated by design (task #430).

use crate::bytes::word_break;
use crate::keywords::{Closer, Keyword, keyword_kind, wait_opens_block};

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

/// Does `word` — a word the scan has already scored as a **block opener** — open
/// a block whose clause ends with an arm `)`: `case`, or the event-`wait` block
/// (fdshell's extension, whose arm mirrors a `case` arm)? Only such a block's
/// `)`-ending word is the clause's end; in an `if`/`while`/`until`/`for` clause
/// such a word is a subshell tail or an argument, and the clause stays open
/// (task #430).
///
/// The predicate is the mode stack's row alone — [`super::after`]'s `)` row
/// answers command position and stays ungated — and it reads the **one**
/// recognition table ([`crate::keywords::keyword_kind`]) so the boundary rule,
/// the quote gate and the `time [-p]` strip are shared, not re-derived. The table
/// maps the event-`wait` block to `Done`, shared with `for`/`while`/`until`, so
/// the `Closer` alone cannot tell it apart: that row re-runs the table's own
/// [`crate::keywords::wait_opens_block`] disambiguation.
pub(crate) fn arm_clause_block(word: &[u8], line: &[u8], word_end: usize) -> bool {
    match keyword_kind(word, line, word_end) {
        Some(Keyword::Opens(Closer::Esac)) => true,
        Some(Keyword::Opens(Closer::Done)) => wait_opens_block(line, word_end),
        _ => false,
    }
}
