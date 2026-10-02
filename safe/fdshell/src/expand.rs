mod fields;

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
    words_quoted: &[bool],
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ImportedStr>, Report<ResolveError>> {
    let mut out = Vec::new();
    for (i, word) in words.iter().enumerate() {
        let bs = word.as_bytes().change_context(ResolveError::RefNotFound)?;
        if fields::is_arith(bs) {
            out.push(expand_arith_word(bs, text, cell)?);
        } else if fields::is_cmd_subst(bs) {
            let expanded = crate::cmd_subst::run_and_capture(fields::strip_delims(bs), cell)
                .change_context(ResolveError::Resolve)?;
            let quoted = words_quoted.get(i).copied().unwrap_or(false);
            // bash: a quoted for-list word that expands to nothing is one
            // empty word (`for w in "$(true)"`); unquoted, it vanishes.
            let mut fields = fields::split_whitespace(&expanded)?;
            if fields.is_empty() && quoted {
                fields.push(ShortCStr::new());
            }
            for w in fields {
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

#[cfg(test)]
mod tests;
