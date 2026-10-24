//! The keyword **gate** of the command-position rule (task #236): which place
//! lets a keyword-shaped word score as a block opener/closer. The transitions
//! that produce the place live in [`super::after`]; the list kinds in
//! [`super::list`] and [`super::clause`].

use crate::keywords::keyword_delta;
use crate::segment::group::{closes, opens};

use super::{Place, list};

/// May the keyword word be scored at `place`? An opener scores in command
/// position only; a closer in command position *or* at a `case` list — bash
/// requires a separator before a closing keyword (`done echo x` is a syntax
/// error), so a closer in argument position is word data.
///
/// At [`Place::Words`] — a `for` word list — nothing scores at all
/// ([`list::scores`]): the list's terminator is `do`, a non-closer, so bash
/// reads every word of it as a word (`for i in done` iterates over `done`,
/// `for i in esac` over `esac`; task #306).
///
/// A brace word is never gated: a literal `{`/`}` argument eating the block's
/// closer is the accepted divergence
/// `tests/group.rs::literal_brace_argument_in_a_block_body` pins; its
/// clause-position rule belongs to tasks #189/#193. The `for` **list** gates
/// braces (bash runs `for i in { 1 2` and `for i in }; do echo x; done`), which
/// is a list rule, not that body divergence.
pub(crate) fn scores(word: &[u8], line: &[u8], word_end: usize, place: Place) -> bool {
    if !list::scores(place) {
        return false;
    }
    if opens(word, line, word_end) || closes(word) {
        return true;
    }
    match keyword_delta(word, line, word_end) {
        Some(delta) if delta > 0 => matches!(place, Place::Command),
        Some(delta) if delta < 0 => matches!(place, Place::Command | Place::Pattern),
        _ => false,
    }
}
