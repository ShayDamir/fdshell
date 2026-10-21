//! One word expanded to its **fields**: substitution, then IFS splitting —
//! the pre-glob half of the argument pipeline. `substitute/words.rs` globs
//! every field; the redirect target calls this directly because it globs at
//! its own base with its own policy (task #278).

use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

use super::arg::substitute_arg_marked;
use super::borrow_state;
use super::positional::expand_positional_word;
use super::split::split_word;

/// One word as the field list a caller globs: `$( … )`, `` ` … ` ``,
/// `$(( … ))`, `$name`, `%fd` and `~` are substituted, the result is split on
/// `IFS` cutting only expansion-produced bytes (POSIX 2.6.5), and each field
/// keeps the quote mask of its own span. A fully quoted word is one field,
/// kept even when the expansion is empty; a word that carried quotes and
/// produced no field at all is the one empty field.
///
/// The fields are **not** pathname-expanded here: the caller owns the glob
/// (arguments glob CWD-rooted with the `nullglob`/`failglob` policy, a
/// redirect target globs at the strict `%CWD` base and keeps its literal
/// pattern on no match).
pub(crate) fn word_fields(
    word: &ShortCStr,
    mask: &[bool],
    spans: &[usize],
    quoted: bool,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<(ShortCStr, Vec<bool>)>, Report<ResolveError>> {
    let fq = super::fully_quoted(mask);
    if word.eq_bytes(b"$@") || word.eq_bytes(b"$*") {
        return positional_fields(word.eq_bytes(b"$*"), fq, quoted, cell);
    }
    let (text, text_mask, origin) = substitute_arg_marked(word, mask, spans, cache, cell)?;
    // A fully quoted word is exactly one word — kept even when the expansion
    // is empty (`"${Y:+a}"` → one empty argument).
    let mut fields = if fq && text.is_empty() {
        vec![(text, text_mask)]
    } else {
        // Field splitting cuts only the bytes the provenance marks as
        // expansion-produced: a word with no expansion has no delimiter
        // bytes, and a fully quoted word has every byte protected, so the
        // splitter returns it as one field. A quoted word whose expansion is
        // empty has no bytes to split at all — the splitter yields no field,
        // and the rule below restores the one empty field.
        //
        // The IFS borrow must end before the caller's glob reborrows the cell
        // for `nullglob` (RefCell, LESSONS.md).
        let state = borrow_state(cell)?;
        split_word(&text, &text_mask, &state.ifs, &origin)?
    };
    push_quoted_empty(&mut fields, quoted);
    Ok(fields)
}

/// The whole-word `$@` / `$*` arm: bash's `"$@"` **is** the positional words,
/// not a word that expanded to nothing — with no positionals it yields no
/// field at all, so the quoted-empty rule must not fire on it (task #219).
/// `$*` keeps its single joined word (one empty word when there are none), and
/// a partially quoted `""$@` (mask all-`false`, so `fq` false) keeps the rule.
fn positional_fields(
    is_star: bool,
    fq: bool,
    quoted: bool,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<(ShortCStr, Vec<bool>)>, Report<ResolveError>> {
    let mut fields = expand_positional_word(is_star, fq, cell)?;
    if fq && !is_star {
        return Ok(fields);
    }
    push_quoted_empty(&mut fields, quoted);
    Ok(fields)
}

/// bash: a word that contained quotes but expanded to no words is one empty
/// word (`""$(true)`, `""$@` with no positionals). The empty field carries the
/// empty mask, which `fully_quoted` reads as quoted, so the caller's glob
/// leaves it alone.
fn push_quoted_empty(fields: &mut Vec<(ShortCStr, Vec<bool>)>, quoted: bool) {
    if quoted && fields.is_empty() {
        fields.push((ShortCStr::new(), Vec::new()));
    }
}
