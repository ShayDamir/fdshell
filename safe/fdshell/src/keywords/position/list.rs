//! The `for` word list (task #306): its own [`Place::Words`].
//!
//! A `for` list has **no command position at all** — its terminator is `do`, a
//! non-closer — so bash reads every word of it as a word: `for i in done`
//! iterates over `done`, `for i in esac` over `esac`, and `for i in { 1 2` over
//! the three words `{`, `1`, `2`. The `case` list is the opposite kind (its
//! terminator `esac` *is* a closer, so `case x in esac` is a complete `case`),
//! which is why the two lists are two places. The keyword that tells them apart
//! is the **opener**, not `in`: `for` in command position opens this place, the
//! `in` behind a `case` opens [`Place::Pattern`].
//!
//! The rule is read by every scorer that walks block text (the
//! `comment/depth_delta` span scan, the `cond/boundary/block` counter, the
//! `parse/semi/closing` keyword walk) because they all consume [`super::scores`]
//! and [`super::after`]: a word-list rule fixed at one of them alone is a
//! divergence at the others (LESSONS, task #273). The block-span scan needs one
//! more half — a block's **own** list has no opener word inside the scanned
//! text, because the scan starts past it, so the segment layer seeds that run
//! with this place ([`crate::segment::block`], and the seed is read by the run
//! the scan begins in: a word list never crosses a separator).

use crate::keywords::position::separator_shape;
use crate::keywords::{Glue, Place};

/// May a keyword word be scored at all at `place`? A word of a `for` list never
/// is. The gate runs **before** the brace carve-out of [`super::scores`], so a
/// brace word of the list is word data too: bash runs `for i in }; do echo x;
/// done` and `for i in { 1 2; do echo "$i"; done`. The body-position brace
/// divergence (`tests/group.rs::literal_brace_argument_in_a_block_body`) stays
/// ungated — it is a command-position rule, not a list rule.
pub(crate) fn scores(place: Place) -> bool {
    !matches!(place, Place::Words)
}

/// The place the word behind a word of a `for` list sits in: the list **stays**
/// a list, so no keyword-shaped word of it is ever scored. It leaves only at a
/// separator (the `;`/newline that closes its run, the `&&`/`||` of the accepted
/// list divergence, a substitution's paren) and at `do`, the keyword that opens
/// the body and terminates the list. `then`/`else` stay list words: bash runs
/// `for i in then; do echo "$i"; done` and prints `then`.
pub(crate) fn step(w: &[u8], glue: Glue) -> Place {
    if matches!(glue, Glue::None) && (separator_shape(w) || w == b"do") {
        return Place::Command;
    }
    Place::Words
}
