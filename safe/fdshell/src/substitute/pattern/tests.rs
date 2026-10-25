//! The `case` pattern surface (task #319): the `$@`/`$*` arm matrix on the
//! pattern text **and** its mask, the boundary-only cut, and the take-first
//! rule. Every row is measured in bash 5.3.9 and mirrors a pin of
//! `tests/case.rs`; the shapes are the tokenizer's — the quote bytes are
//! stripped, the mask marks each region's start with `QuotedStart`.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use alloc::vec;
use alloc::vec::Vec;
use core::ffi::CStr;
use hashbrown::HashMap;

use sys::ExportedFd;
use sys::ImportedStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::mask::MaskBit;
use crate::mask::MaskBit::{Quoted, QuotedStart, Unquoted};
use crate::state::ShellState;

use super::word;

/// The pattern surface as `case_exec` runs it: `ifs` is the shell's `IFS`,
/// `positional` the `$1..$N`. Every returned pattern is checked for the one
/// invariant the matcher depends on: the mask is parallel to the text.
fn pat(
    text: &'static CStr,
    mask: &[MaskBit],
    ifs: ShortCStr,
    positional: &[&'static CStr],
) -> (ShortCStr, Vec<MaskBit>) {
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.ifs = ifs;
        state.set_positional(
            positional
                .iter()
                .map(|p| ImportedStr::shell(ShortCStr::from(*p)))
                .collect(),
        );
    }
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let text = ShortCStr::from(text);
    let (text, text_mask) = word(&text, mask, &[], &mut cache, &cell).unwrap();
    assert_eq!(
        text_mask.len(),
        text.len(),
        "the mask is parallel to the kept pattern"
    );
    (text, text_mask)
}

fn bytes(text: &ShortCStr) -> Vec<u8> {
    text.as_bytes().unwrap().to_vec()
}

/// `set -- a b; case a in "$@")` matches and `case b in "$@")` does not: bash
/// keeps only the **first expanded word** (`es->word->word`), so a quoted
/// `"$@"` is `p1`, and its bytes inherit the trigger's quote bit.
#[test]
fn quoted_dollar_at_pattern_is_the_first_positional_only() {
    let pos = [c"a", c"b"];
    let (text, mask) = pat(
        c"$@",
        &[QuotedStart, Quoted],
        ShellState::default_ifs(),
        &pos,
    );
    assert_eq!(bytes(&text), b"a");
    assert_eq!(mask, vec![Quoted]);
}

/// `IFS=,; set -- "a b" c; case "xa b" in x"$@"y)` matches, `case cy in x"$@"y)`
/// does not: the glue touching `p1` survives, the rest of the word list is
/// dropped with `p2` and its glue.
#[test]
fn quoted_dollar_at_pattern_keeps_the_glue_of_the_first_piece() {
    let pos = [c"a b", c"c"];
    let mask = [Unquoted, QuotedStart, Quoted, Unquoted];
    let ifs = ShortCStr::from(c",");
    let (text, out_mask) = pat(c"x$@y", &mask, ifs, &pos);
    assert_eq!(bytes(&text), b"xa b");
    assert_eq!(out_mask, vec![Unquoted, Quoted, Quoted, Quoted]);
}

/// `set -- a ""; case xa in x"$@"y)` matches and `set -- "" b; case x in
/// x"$@"y)` matches: the boundary of an empty positional still cuts the word,
/// so the kept piece is the text up to it.
#[test]
fn empty_positionals_cut_the_pattern_at_their_boundaries() {
    let mask = [Unquoted, QuotedStart, Quoted, Unquoted];
    let (text, _) = pat(c"x$@y", &mask, ShellState::default_ifs(), &[c"a", c""]);
    assert_eq!(bytes(&text), b"xa");
    let (text, _) = pat(c"x$@y", &mask, ShellState::default_ifs(), &[c"", c"b"]);
    assert_eq!(bytes(&text), b"x");
}

/// `IFS=; set -- a b; case "a b" in "$@")` matches (and `case a in "$@")` does
/// not): with an empty `IFS` the arm emits **no boundary**, so nothing is
/// dropped — the space join *is* the pattern.
#[test]
fn quoted_dollar_at_pattern_space_joins_with_an_empty_ifs() {
    let (text, mask) = pat(
        c"$@",
        &[QuotedStart, Quoted],
        ShortCStr::new(),
        &[c"a", c"b"],
    );
    assert_eq!(bytes(&text), b"a b");
    assert_eq!(mask, vec![Quoted, Quoted, Quoted]);
}

/// `IFS=,; set -- a b; case "pa bq" in p$@q)` matches: an unquoted `$@`
/// space-joins whatever the `IFS` is, glue kept, and no boundary is emitted so
/// the whole text is the pattern.
#[test]
fn unquoted_dollar_at_pattern_space_joins_whatever_the_ifs_is() {
    let ifs = ShortCStr::from(c",");
    let (text, mask) = pat(c"p$@q", &[], ifs, &[c"a", c"b"]);
    assert_eq!(bytes(&text), b"pa bq");
    assert_eq!(mask, vec![Unquoted; 5]);
}

/// `IFS=,; set -- a b; case "a,b" in $*)` and `"$*")` both match: `$*` is one
/// word joined with the **first `IFS` byte**, quoted or not.
#[test]
fn dollar_star_pattern_joins_with_the_first_ifs_byte() {
    let (text, out_mask) = pat(c"$*", &[], ShortCStr::from(c","), &[c"a", c"b"]);
    assert_eq!(bytes(&text), b"a,b");
    assert_eq!(out_mask, vec![Unquoted; 3]);
    let (text, out_mask) = pat(
        c"$*",
        &[QuotedStart, Quoted],
        ShortCStr::from(c","),
        &[c"a", c"b"],
    );
    assert_eq!(bytes(&text), b"a,b");
    assert_eq!(
        out_mask,
        vec![Quoted; 3],
        "a quoted `$*` keeps its glob chars literal"
    );
}

/// `IFS=; set -- a b; case ab in $*)` matches: an empty `IFS` **concatenates**
/// — the word path's space-when-empty separator is the one row the pattern
/// surface must not share (task #319).
#[test]
fn dollar_star_pattern_concatenates_with_an_empty_ifs() {
    let (text, mask) = pat(c"$*", &[], ShortCStr::new(), &[c"a", c"b"]);
    assert_eq!(bytes(&text), b"ab");
    assert_eq!(mask, vec![Unquoted; 2]);
}

/// The mask is the pattern's quoting signal (task #280): a quoted positional's
/// glob chars stay **literal**, an unquoted one's stay **live** —
/// `set -- 'a*b'; case "a*b" in "$@")` matches and `case a*b in "$@")` does
/// not, while `set -- 'a*b'; case aXb in $@)` matches.
#[test]
fn a_positionals_glob_chars_follow_the_trigger_quote_bit() {
    let pos = [c"a*b"];
    let (text, mask) = pat(
        c"$@",
        &[QuotedStart, Quoted],
        ShellState::default_ifs(),
        &pos,
    );
    assert_eq!(bytes(&text), b"a*b");
    assert_eq!(mask, vec![Quoted; 3], "a quoted `*` is a literal byte");
    let (text, mask) = pat(c"$@", &[], ShellState::default_ifs(), &pos);
    assert_eq!(bytes(&text), b"a*b");
    assert_eq!(mask, vec![Unquoted; 3], "an unquoted `*` globs");
}

/// `set --; case "" in "$@")` matches: no positionals is the **empty** pattern
/// with the empty mask, which matches only the empty word.
#[test]
fn no_positionals_is_the_empty_pattern_with_an_empty_mask() {
    let (text, mask) = pat(
        c"$@",
        &[QuotedStart, Quoted],
        ShellState::default_ifs(),
        &[],
    );
    assert_eq!(bytes(&text), b"");
    assert!(mask.is_empty());
}

/// `set --; case xy in x"$@"y)` matches: an expansion that produced nothing
/// leaves its glue, which is the whole word.
#[test]
fn no_positionals_leaves_the_glue() {
    let mask = [Unquoted, QuotedStart, Quoted, Unquoted];
    let (text, out_mask) = pat(c"x$@y", &mask, ShellState::default_ifs(), &[]);
    assert_eq!(bytes(&text), b"xy");
    assert_eq!(out_mask, vec![Unquoted; 2]);
}

/// A pattern is never IFS-split (task #280's rule, `v="x y"; case y in $v)`
/// does not match): a positional carrying a space keeps it whole even under a
/// whitespace `IFS`, because the pattern's cut is a boundary cut only.
#[test]
fn a_positional_with_spaces_survives_the_cut() {
    let (text, mask) = pat(
        c"$@",
        &[QuotedStart, Quoted],
        ShellState::default_ifs(),
        &[c"a b"],
    );
    assert_eq!(bytes(&text), b"a b");
    assert_eq!(mask, vec![Quoted; 3]);
}
