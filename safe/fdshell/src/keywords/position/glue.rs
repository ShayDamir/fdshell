//! The operator-strip half of a glued word (task #236's S2): the count of
//! leading operator bytes a [`Glue`] position carries. Words outside a cond
//! pair strip nothing; their place rule is the fall-through of [`super::after`].

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
