//! The **reserved words** the metacharacter boundary of task #396 splits at,
//! one list built from what the recognizers already match: the block
//! openers and closers of [`crate::keywords::kind`] (`case`/`esac`, `if`/`fi`,
//! `for`/`while`/`until`/`done`) plus the clause sets of
//! [`crate::keywords::position::clause`] (`then`/`else`/`elif`, `do`, `in`).
//! The tokenizer's keyword split ([`crate::parse::token::step::keyword`])
//! reads this list and nothing else does, so the words it covers are exactly
//! the words both granularities recognize.
//!
//! Two recognizers' words stay **out**: `wait` is not a bash reserved word
//! (its boundary row is the event-block disambiguation, and bash reads a glued
//! `wait(` as the general word break — task #316's), and `time` is a *prefix*,
//! not a keyword word ([`crate::keywords::time_prefix_len`]). The group's
//! `{`/`}` are neither: bash's word break at `{`/`}` is not a metacharacter,
//! and the group scanner owns those words.

/// The word `w` is a bash reserved word the boundary rule applies to — the
/// bytes the tokenizer has accumulated, compared whole. The one list; the
/// recognizers it names are its other readers.
pub(crate) fn reserved_word(w: &[u8]) -> bool {
    matches!(
        w,
        b"case"
            | b"esac"
            | b"if"
            | b"fi"
            | b"for"
            | b"while"
            | b"until"
            | b"done"
            | b"then"
            | b"else"
            | b"elif"
            | b"do"
            | b"in"
    )
}

#[cfg(test)]
mod tests;
