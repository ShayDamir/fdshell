//! Word-level substitution: per-argument expansion, IFS splitting, globbing.

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

pub fn substitute_args(
    args: &[ShortCStr],
    args_mask: &[Vec<bool>],
    args_quoted: &[bool],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let mut result = Vec::new();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    for (i, arg) in args.iter().enumerate() {
        let mask = args_mask.get(i).cloned().unwrap_or_default();
        let quoted = args_quoted.get(i).copied().unwrap_or(false);
        let before = result.len();
        if arg.eq_bytes(b"$@") || arg.eq_bytes(b"$*") {
            let is_star = arg.eq_bytes(b"$*");
            let fq = super::fully_quoted(&mask);
            for (word, mask) in expand_positional_word(is_star, fq, cell)? {
                push_word(&mut result, &word, &mask, cell)?;
            }
            // bash: a fully quoted whole-word `$@` *is* the positional words,
            // not a word that expanded to nothing — with no positionals it
            // yields no word at all, so the quoted-empty rule below must not
            // fire on it (task #219). `$*` keeps its single joined word (one
            // empty word when there are none), and a partially quoted `""$@`
            // (mask all-`false`, so `fq` false) keeps the rule.
            if fq && !is_star {
                continue;
            }
        } else {
            // A fully quoted word is exactly one word — kept even when the
            // expansion is empty (`"${Y:+a}"` → one empty argument).
            let fq = super::fully_quoted(&mask);
            let mut expanded = false;
            let (text, mask) = substitute_arg_marked(arg, &mask, &mut cache, cell, &mut expanded)?;
            // Field splitting applies to the result of expansion only (POSIX
            // 2.6.5): a word that performed no expansion is one field, so a
            // literal `IFS` byte inside it never delimits.
            let fields = if fq || !expanded {
                vec![(text, mask)]
            } else {
                // The IFS borrow must end before the glob reborrows the cell
                // for `nullglob` (RefCell, LESSONS.md).
                let state = borrow_state(cell)?;
                split_word(&text, &mask, &state.ifs)?
            };
            for (word, mask) in fields {
                push_word(&mut result, &word, &mask, cell)?;
            }
        }
        // bash: a word that contained quotes but expanded to no words is one
        // empty word (`""$(true)`, `""$@` with no positionals). Push it
        // directly rather than through `push_word` — an empty mask means
        // "fully quoted" there, and passing the expansion mask through would
        // glob it.
        if quoted && result.len() == before {
            result.push(ShortCStr::new());
        }
    }
    Ok(result)
}

/// One word after IFS splitting: a fully quoted word is kept as-is (even
/// empty), any other word is pathname-expanded first.
fn push_word(
    result: &mut Vec<ShortCStr>,
    word: &ShortCStr,
    mask: &[bool],
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<ResolveError>> {
    if super::fully_quoted(mask) {
        result.push(word.clone());
        return Ok(());
    }
    for expanded in crate::glob::expand(word, mask, cell)? {
        result.push(expanded);
    }
    Ok(())
}
