//! The two flags the command-position rule carries: [`Place`] — where the word
//! written next sits — and [`Glue`] — where a word starts inside a glued
//! `&&`/`||` cond pair. Both are data; the transitions are [`super::after`] and
//! the payload preprocessing that reads the glue is [`super::payload`].

/// Where the word written next sits, as far as the keyword scorers can tell.
/// The state transitions are [`super::after`]; the gate is [`super::scores`].
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(crate) enum Place {
    /// A command may start here: the run's start, behind a cond operator,
    /// after `then`/`do`/`else`/`elif`, after a condition-taking opener, a
    /// group opener, a `case` clause's `)` or a **closer** in command position
    /// (bash's grammar makes the `;` after a compound command optional, so
    /// `while :; do if :; then :; fi done` is one command list). Only here does
    /// a block keyword score.
    Command,
    /// A `case` word list (`case x in …`). Its patterns are words, so no
    /// opener scores there, and `esac` is the list's terminator, so a closer
    /// does (`case x in esac` is a complete `case`).
    Pattern,
    /// A `for` word list (`for i in …`, task #306). Its terminator is `do`, a
    /// non-closer, so the list has **no** command position at all: no keyword
    /// scores here — not a closer-shaped word (`for i in done`), not an opener,
    /// not a brace word (`for i in { 1 2`, `for i in }`). The place is sticky;
    /// [`super::list::step`] leaves it only at a separator (which ends the list)
    /// or at the `do` that opens the body.
    Words,
    /// An argument position: every word here is data.
    Data,
}

/// Where a word starts inside a glued `&&`/`||` cond pair (task #236's S2).
/// The pair is *two* word starts: [`is_word_break`](crate::scan::advance)
/// breaks words at operator bytes, so `p||if` is scanned as the words `p`,
/// `||if` (the pair's head) and `|if` (its tail), and the tokenized `p||if`
/// carries the bare halves as separate tokens. A *pipeline* `|` is a
/// single-byte run, so its word carries [`Glue::None`] and the plain rule of
/// [`super::after`] scores the stage behind it (task #292).
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(crate) enum Glue {
    /// Outside a cond pair.
    None,
    /// At the pair's first byte: the payload after the pair is in command
    /// position.
    Head,
    /// At a later byte of the pair: the word continues the pair's command
    /// position (bare tail: transparent, see [`super::after`]).
    Tail,
}
