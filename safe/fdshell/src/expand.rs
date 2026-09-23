use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Origin, ScriptText, Trace};

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub(crate) fn expand_for_words(
    words: &[ShortCStr],
    words_mask: &[Vec<bool>],
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ImportedStr>, Report<ResolveError>> {
    let mut out = Vec::new();
    for (i, word) in words.iter().enumerate() {
        let bs = word.as_bytes().change_context(ResolveError::RefNotFound)?;
        if is_arith(bs) {
            out.push(expand_arith_word(bs, text, cell)?);
        } else if is_cmd_subst(bs) {
            let expanded = crate::cmd_subst::run_and_capture(strip_delims(bs), cell)
                .change_context(ResolveError::Resolve)?;
            for w in split_whitespace(&expanded)? {
                out.push(ImportedStr::new(
                    w,
                    Trace::at(text.start, Origin::CommandOutput),
                ));
            }
        } else {
            // Literal for-list words are pathname-expanded (whole-word
            // `$((…))`/`$(…)` are exempt); each result is a new shell word.
            let mask = words_mask.get(i).cloned().unwrap_or_default();
            for w in crate::glob::expand(word, &mask, cell)? {
                out.push(ImportedStr::new(w, Trace::at(text.start, Origin::Shell)));
            }
        }
    }
    Ok(out)
}

/// Whole-word `$((expr))` in a for list: evaluate in-process; the decimal
/// result is one word (no splitting, unlike command substitution).
fn expand_arith_word(
    bs: &[u8],
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<ImportedStr, Report<ResolveError>> {
    let body = bs.get(3..bs.len() - 2).unwrap_or(b"");
    let value = crate::arith::eval(body, cell).change_context(ResolveError::Resolve)?;
    let rendered = crate::arith::render(value)?;
    Ok(ImportedStr::new(
        rendered,
        Trace::at(text.start, Origin::Shell),
    ))
}

fn is_arith(bs: &[u8]) -> bool {
    bs.len() >= 4 && bs.starts_with(b"$((") && bs.ends_with(b"))")
}

fn is_cmd_subst(bs: &[u8]) -> bool {
    (bs.first() == Some(&b'`') && bs.last() == Some(&b'`') && bs.len() >= 2)
        || (bs.len() >= 3 && bs.starts_with(b"$(") && bs.last() == Some(&b')'))
}

fn strip_delims(bs: &[u8]) -> &[u8] {
    if bs.starts_with(b"$(") {
        bs.get(2..bs.len() - 1).unwrap_or(b"")
    } else {
        bs.get(1..bs.len() - 1).unwrap_or(b"")
    }
}

fn split_whitespace(data: &[u8]) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let mut words = Vec::new();
    let mut cur = ShortCStr::new();
    for &b in data {
        if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
            if !cur.is_empty() {
                words.push(core::mem::take(&mut cur));
            }
        } else {
            cur.push_byte(b).change_context(ResolveError::NulByte)?;
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    Ok(words)
}

#[cfg(test)]
mod tests;
