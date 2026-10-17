//! The operator-strip halves of a glued word (task #236's S2): the count of
//! leading operator bytes a [`Glue`] position carries, and the payload of a
//! word whose operator run is a single byte (a pipeline tail, pair forms
//! included) — the strip `after` reads the payload behind.

use super::Glue;

/// The number of leading operator bytes to strip at a glue position: a
/// pair's head carries both bytes, its tail one, a word outside a pair
/// none.
pub(super) fn strip(glue: Glue) -> usize {
    match glue {
        Glue::None => 0,
        Glue::Tail => 1,
        Glue::Head => 2,
    }
}

/// The payload behind a *single* operator byte of a word *not* in a cond
/// pair ([`Glue::None`]): pipeline tails (`|if`) and glued operator forms
/// (`&>`) classify by it. A cond pair strips by [`strip`], never here.
/// `None` for a word that starts outside an operator run.
pub(super) fn tail(word: &[u8]) -> Option<&[u8]> {
    let head = *word.first()?;
    if head != b'&' && head != b'|' {
        return None;
    }
    let run = [head];
    word.strip_prefix(&run[..])
        .map(|t| t.strip_prefix(&run[..]).unwrap_or(t))
        .map(|t| t.trim_ascii())
}
