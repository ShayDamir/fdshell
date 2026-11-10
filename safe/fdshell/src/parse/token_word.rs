// mutants: -C --lib -C --test -C case -C --test -C semi_semi -C --test -C redirect -- -E 'test(/^parse::token_word::/) | test(/^keywords::position::/) | test(/^parse::/) | binary(case) | binary(semi_semi) | binary(redirect)'

//! The parser's word token as a named-field struct — the six slots of the
//! former `parse.rs` tuple alias, each named — plus the predicates whose only
//! data is those slots (task #549 gave the type its fields, task #550 gives it
//! its methods). A method lives here when its body needs nothing beyond the
//! fields, `core`/`alloc`, `sys`, [`crate::mask`] and a `pub(crate)` predicate
//! imported from elsewhere in the crate; a method that returns another module's
//! type or calls that module's private helper keeps its `impl Token` block in
//! that owner module (the shape `ShortCStr` already uses inside its own family).

use crate::keywords::separator_shape;
use crate::mask::{MaskBit, quoted_at};
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

/// A token: its word, the raw byte range it was cut from, and the quote
/// bookkeeping that survived quote-stripping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    /// The quote-stripped word (empty for a quoted empty word such as `""`).
    pub(crate) word: ShortCStr,
    /// Start of the byte range of the token's raw text.
    pub(crate) start: usize,
    /// End (exclusive) of that same raw-text byte range.
    pub(crate) end: usize,
    /// Whether the word was fully quoted.
    pub(crate) fully_quoted: bool,
    /// The per-byte quote mask, parallel to the word; `true` marks bytes that
    /// were inside quotes (double **or** single) and are protected from IFS
    /// word splitting.
    pub(crate) mask: Vec<MaskBit>,
    /// The word-text indices at which a quote span closed **without
    /// contributing any byte** (task #321: the mask cannot see a `""` / `''`
    /// pair — zero bytes, zero bits — so the tilde rule consults this list;
    /// `~""/x` → text `~/x`, mask all-`false`, spans `[1]`). Parallel
    /// plumbing: every surface that carries a mask carries this list at the
    /// same offset.
    pub(crate) spans: Vec<usize>,
}

impl Token {
    /// The separator-recognition gate (POSIX §2.9.1, task #336): `self` **is**
    /// the separator word `w` only when its raw span is exactly those bytes, so
    /// it carries no quote byte. A quote-protected `";"`, `';'`, `"|"`, `"&"`,
    /// `")"` is a word whose *text* is the separator, and bash reads it as word
    /// **data**: it splits no statement, no pipeline, no `case` clause, and it
    /// never restarts command position (`echo a ";" b` prints `a ; b`).
    /// LESSONS: "A separator-shaped word needs the span gate"
    ///
    /// The gate is the raw span, never the mask (`"";` is an empty word token
    /// **plus** a real `;`, and `a";"b` is one word whose text is `a;b`) and
    /// never [`crate::keywords::quoted_word`]'s scan-back, whose word-break set
    /// contains the separator bytes themselves, so it is sound only for a
    /// keyword word, which ends in a letter. The span form is also the gate the
    /// sites that own no `line` can compute ([`Token::is_semi`],
    /// `pipeline/stage.rs::trim_at_semi`, [`crate::parse::envassign`]).
    /// Escape spellings need no arm: `\;`, `\|`, `\)` keep the pair in the token
    /// **text** (`parse/token/escape.rs`), so no separator text exists to gate.
    /// Byte-level twin: the byte scanners read raw bytes, where a quoted
    /// separator's word text is `";"` / `"|"` and no separator row can fire —
    /// `parse::redirect::clobber::pipe::pipe_token_span` is this rule for the
    /// `|`.
    pub(crate) fn span_word(&self, w: &[u8]) -> bool {
        self.word.eq_bytes(w) && !self.word.is_shorter_than_span(self.start, self.end)
    }

    /// The `;` **token**: the tokenizer emits one for a `;` byte, a newline and
    /// `)` alike, so this is the token test, never a byte test
    /// ([`crate::parse::semi::amp_semi`] adds the byte), and
    /// [`Token::span_word`] is the quoting half — a quoted `";"` is word data
    /// and trims out of no slice (task #336).
    pub(crate) fn is_semi(&self) -> bool {
        self.span_word(b";")
    }

    /// A command-list terminator: POSIX's `;` (a newline is tokenized as one) or
    /// the plain `&` that ends the statement it backgrounds (task #354). Both
    /// through [`Token::span_word`], so a quoted `";"` / `"&"` word terminates
    /// nothing (task #336) — the rule `group_block.rs` (the `;` before `}`) and
    /// [`crate::parse::semi::is_closer_predecessor`] read.
    pub(crate) fn is_list_terminator(&self) -> bool {
        self.span_word(b";") || self.span_word(b"&")
    }

    /// `self` is a **separator-shaped word**: its text is one of
    /// [`separator_shape`]'s words **and** its raw span carries quote bytes, so
    /// POSIX §2.9.1 makes it word data and not the separator
    /// [`crate::keywords::after`]'s rows read (task #336 — the
    /// token-level twin of [`Token::span_word`], which is text **and** span,
    /// reached through `ShortCStr::is_shorter_than_span`). The text is trimmed as
    /// [`crate::keywords::after`] trims it, because a quoted `" ; "`
    /// word carries its spaces in the text.
    pub(crate) fn separator_data(&self) -> bool {
        self.word.as_bytes().is_ok_and(|w| {
            separator_shape(w.trim_ascii()) && self.word.is_shorter_than_span(self.start, self.end)
        })
    }

    /// The operator words the tokenizer emits as tokens of their own — every one
    /// of them through [`Token::span_word`], so a quoted spelling is a word,
    /// never an operator predecessor (task #336).
    pub(crate) fn is_operator(&self) -> bool {
        self.span_word(b";") || self.span_word(b"|") || self.span_word(b"&") || self.span_word(b")")
    }

    /// The glue test of a terminator pair: `self`'s span ends where `other`'s
    /// begins (the pair rule `h.2 == t.1`, pinned by `tests/glued_pair.rs` and
    /// the #355 pins).
    pub(crate) fn glued(&self, other: &Self) -> bool {
        self.end == other.start
    }

    /// The token that opens a clause's pattern list, with bash's optional leading
    /// `(` taken off (POSIX 2.9.4.3 spells a clause `( pattern-list )`, bash 5.3.9
    /// strips the `(`). `None`: the token carries no such `(`, so the clause walk
    /// starts where it was. `Some(tokens)`: the byte was taken, and `tokens` is what
    /// is left of the token — **empty** when the token *is* the `(` (POSIX's spaced
    /// `( pattern )`), so `case x in ()` keeps fdshell's empty-pattern verdict and
    /// [`crate::parse::semi::try_join_masked`] gains no leading space.
    ///
    /// The byte is taken only when that byte's mask bit is `Unquoted` **and** the
    /// text starts with `(` — the same two signals
    /// `stray::opens_groups` reads for a paren: a
    /// quoted `"(a*` / `'('a*` keeps the bit (measured: no strip, bash
    /// agrees), and an escaped `\(a*` keeps the whole pair in the text, so the paren
    /// is not the leading byte. A `(` behind a `|` is never reached: the caller asks
    /// about the clause's **first** token only, and one byte is taken at that (the
    /// `((a*))` shape keeps its inner `(`).
    ///
    /// Text, mask, spans and the raw span move together, so the slice stays
    /// self-consistent for [`crate::parse::semi::try_join_masked`] (the mask's only
    /// owner) and for the raw-span test [`Token::span_word`] is: byte 0 goes, bit 0
    /// goes, every span index steps back one (a span at 0 is a quote pair written in
    /// front of the `(` and stays at 0), and `start` follows the text.
    pub(crate) fn take_open_paren(&self) -> Option<Vec<Token>> {
        // The mask is the discriminator, never the per-word `fully_quoted` flag:
        // only an unquoted leading `(` is the token POSIX writes, the rest is text.
        if quoted_at(&self.mask, 0) {
            return None;
        }
        // The byte itself: an escaped `\(a*` carries the pair's `\` at index 0, so
        // there is no leading paren to take.
        let stripped = self.word.strip_prefix(b"(")?;
        // The token *is* the `(` (POSIX's spaced `( pattern )`): nothing is left of
        // it, so the caller's pattern list starts empty.
        if stripped.is_empty() {
            return Some(Vec::new());
        }
        let opened: Token = Token {
            word: stripped,
            start: self.start + 1,
            end: self.end,
            fully_quoted: self.fully_quoted,
            mask: self.mask.iter().skip(1).copied().collect(),
            spans: self.spans.iter().map(|&i| i.saturating_sub(1)).collect(),
        };
        Some(vec![opened])
    }

    /// The word-level twin of [`Token::take_open_paren`] (task **#470**): the
    /// word's first byte is `(` **and** that byte's mask bit is unquoted — the
    /// same two signals `take_open_paren` and `stray::opens_groups` read. The
    /// tokenizer keeps `(` an ordinary word byte, so a word that **opens** with
    /// it is the control operator POSIX 2.9.4.3 spells `case WORD in` (and the
    /// word list of POSIX 2.9.1's `for`): the word-opening `(` reaches the
    /// word-slot rule this way. A quoted `"(b` keeps its bit and an escaped
    /// `\(` keeps the backslash as the first byte, so neither fires; a `(`
    /// behind the first byte is word data — the span-gate family (LESSONS:
    /// "A separator-shaped word needs the span gate").
    pub(crate) fn opens_with_open_paren(&self) -> bool {
        !quoted_at(&self.mask, 0) && self.word.as_bytes().is_ok_and(|w| w.first() == Some(&b'('))
    }
}

#[cfg(test)]
mod tests;
