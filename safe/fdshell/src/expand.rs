mod fields;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Origin, ScriptText, ShortCStr, Trace};

use crate::error::resolve::ResolveError;
use crate::run_origin::has_cmd_subst;
use crate::state::ShellState;
use crate::substitute::substitute_args;

/// The `for … in` word list, expanded word by word: each word goes through
/// the argument substitution pipeline (tilde, parameter, command — `$(…)` and
/// `` ` … ` `` — arithmetic, `%fd`, `$@`), is IFS-split by its own quote mask,
/// and every field is pathname-expanded — each resulting field is one
/// iteration.
///
/// The list is expanded per word, never whole: `substitute_args` returns a
/// flat field vector, so a whole-list call would lose the word→fields
/// mapping the per-word quote mask, quoted flag and provenance need.
pub(crate) fn expand_for_words(
    words: &[ShortCStr],
    words_mask: &[Vec<bool>],
    words_span: &[Vec<usize>],
    words_quoted: &[bool],
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ImportedStr>, Report<ResolveError>> {
    let mut out = Vec::new();
    for (i, word) in words.iter().enumerate() {
        let mask = words_mask.get(i).cloned().unwrap_or_default();
        let spans = words_span.get(i).cloned().unwrap_or_default();
        let quoted = words_quoted.get(i).copied().unwrap_or(false);
        let bs = word.as_bytes().change_context(ResolveError::RefNotFound)?;
        let origin = word_origin(bs);
        let fields = expand_word(word, &mask, &spans, quoted, cell)?;
        out.extend(
            fields
                .into_iter()
                .map(|field| ImportedStr::new(field, Trace::at(text.start, origin.clone()))),
        );
    }
    Ok(out)
}

/// One for-list word: every word goes through the argument substitution
/// pipeline, so a backtick word is expanded there like every other surface
/// (task #216), and its fields are pathname-expanded by `substitute_args`.
fn expand_word(
    word: &ShortCStr,
    mask: &Vec<bool>,
    spans: &Vec<usize>,
    quoted: bool,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    substitute_args(
        core::slice::from_ref(word),
        core::slice::from_ref(mask),
        core::slice::from_ref(spans),
        core::slice::from_ref(&quoted),
        cell,
    )
}

/// Provenance of a for-list word, classified on its source bytes (user-visible
/// through `explain NAME`): a leading `~` comes from `HOME`, a word carrying a
/// command substitution is command output, anything else is the script.
fn word_origin(bs: &[u8]) -> Origin {
    if bs.first() == Some(&b'~') {
        return Origin::EnvVar(ShortCStr::from(c"HOME"));
    }
    if has_cmd_subst(bs) {
        return Origin::CommandOutput;
    }
    Origin::Shell
}

#[cfg(test)]
mod tests;
