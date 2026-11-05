//! The stray-`;;` rule of task #423: a `;;` outside a `case` clause is a parse
//! error — POSIX #2.10 puts it inside a clause and nowhere else, and bash stops
//! there (fdshell's house rc 1). The rule is **one predicate** — two `;`-class
//! separators with nothing but spaces/tabs between them, the `;&`/`;;&` kinds
//! excluded — read at the two granularities the command lists live at (LESSONS
//! "byte-level and token-level rules must agree"):
//!
//! - [`pair_at_byte`] is the scanner's reader. A `;;` *is* a separator, so the
//!   statement list only shows it to the byte walk: [`crate::scan::boundary`]
//!   answers [`crate::scan::Boundary::SemiSemi`] at the pair's **head** byte and
//!   the segment scan marks the line, so the report is a preflight (bash runs
//!   nothing — the `Segment::AsyncInvalid` family's shape). The byte walk's
//!   [`ScanState`] gate settles quotes, escapes and regions before the predicate
//!   is ever consulted, so the quote rule of task #247/#336 needs no arm here;
//!   the newline gap is no pair (the `;\n;` family belongs to #487), and the
//!   glued `;&`/`;;&` spellings are the terminator kinds, not this rule's pair.
//! - [`first_stray`] is the parse layer's reader. A body is a sub-slice the
//!   scanner never re-splits, so the pair re-appears in its token list: the
//!   shared walk of [`super::walk_until`] with the `case` clause walk's own stop
//!   test (`depth == seed && kind_at(..) == Some(Stop)`), so a `;;` inside a
//!   nested block is skipped and this reader adds no new predicate logic at all —
//!   plus the one rule the tokenizer loses: it writes the newline's `;` as a
//!   separator word, so the reader re-checks the gap ([`real_pair_gap`]) — the
//!   `;\n;` family stays #487's at both granularities. The lazy-body family
//!   governs the rc there (an unrun body is never parsed).

use super::Token;
use crate::parse::case_clause::terminator::{self, Terminator};
use crate::parse::semi::{List, walk_until};
use crate::parse::word_quoted::span_word;
use crate::scan::ScanState;

/// The scanner's reader: is `i` the **head** byte of a stray `;;`? The walk has
/// already settled the quote/escape/region state ([`ScanState`]), so only a
/// bare, depth-0 `;` byte can reach the pair test; the gap is spaces and tabs
/// only (empty = glued), a newline gap is no pair (#487's family), and a `&`
/// glued behind the tail byte is the `;;&` terminator kind, not a pair.
pub(crate) fn pair_at_byte(line: &[u8], i: usize, state: &ScanState) -> bool {
    if state.in_quote || state.in_single || state.in_backtick || state.paren_depth > 0 {
        return false;
    }
    let j = gap_end(line, i + 1);
    line.get(i) == Some(&b';')
        && line.get(j) == Some(&b';')
        && !(j == i + 1 && line.get(j + 1) == Some(&b'&'))
}

/// The parse layer's reader: the first token at which the shared walk's state
/// makes a `;;` a stray one — the walk's top level (seed `None`, so depth 0;
/// [`List::Plain`], so the word-list rule is the shared one). The stop test is
/// exactly the `case` clause walk's, with the terminator kind pinned to `Stop`
/// (`;&`/`;;&` are the other kinds, out of this rule by decision). A hit inside
/// the slice's **leading** separator run — the run `trim_body`/`trim_condition`
/// absorbs — is not a stray one: the leading spellings stay as measured (the
/// empty-body pins own the rest of that family); a pair with kept content
/// before it reports.
pub(crate) fn first_stray(tokens: &[Token], line: &[u8]) -> Option<usize> {
    let lead = tokens.iter().take_while(|t| span_word(t, b";")).count();
    walk_until(tokens, line, None, List::Plain, 0, &|i, depth, seed| {
        depth == seed
            && i >= lead
            && terminator::kind_at(tokens, line, i) == Some(Terminator::Stop)
            && real_pair_gap(tokens, line, i)
    })
}

/// The walk's word stream cannot tell a real `;` from the newline's `;` the
/// tokenizer writes as a separator word; the byte reader's rule is the same
/// predicate: both bytes real, the gap between them spaces/tabs only (empty =
/// glued), so a `;\n;` is no pair here either (#487's family).
fn real_pair_gap(tokens: &[Token], line: &[u8], i: usize) -> bool {
    let head = match tokens.get(i) {
        Some(t) => t,
        None => return false,
    };
    let tail = match tokens.get(i + 1) {
        Some(t) => t,
        None => return false,
    };
    line.get(tail.1) == Some(&b';')
        && line
            .get(head.2..tail.1)
            .is_some_and(|gap| gap.iter().all(|b| matches!(b, b' ' | b'\t')))
}

/// The end of a run of spaces/tabs starting at `from`: the position of the first
/// byte that is not one of them (possibly the end of the line).
fn gap_end(line: &[u8], mut j: usize) -> usize {
    while matches!(line.get(j), Some(&b' ') | Some(&b'\t')) {
        j += 1;
    }
    j
}

#[cfg(test)]
mod tests;
