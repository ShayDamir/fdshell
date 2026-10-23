//! The clause keywords of a keyword run, as one keyword set shared by two
//! rules that must stay in step: [`super::after`]'s command-position
//! transitions and the cond-boundary scan's clause/body mode stack
//! (`cond/boundary/block/clause`). Each predicate is one row of [`super::after`]'s
//! payload match, so a keyword added to the grammar lands in both rules at once.

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
