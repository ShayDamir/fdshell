//! One word-0 word expanded to its bash fields: substitution, IFS splitting,
//! pathname expansion — the same pipeline an argument word goes through.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::bytes::fold::fold_word;
use crate::error::cmd::CmdError;
use crate::state::ShellState;

/// A word whose bytes can start an expansion: `$`, a backtick, a `%`, or a
/// leading `~`. This is a **raw-text** test, never a mask test: `"$(echo ls)"`
/// carries mask `true` on every byte and must still expand (bash doubles
/// quotes do not suppress command substitution), so a mask-gated word is an
/// unsound false negative. A false positive is harmless — every arm of the
/// pipeline is correct on a word with no trigger.
fn has_trigger(bytes: &[u8]) -> bool {
    bytes.first() == Some(&b'~') || bytes.iter().any(|b| matches!(b, b'$' | b'`' | b'%'))
}

/// The word is its own single field: no expansion trigger, no escape pair to
/// fold, no unquoted pattern byte to glob. The fast path that keeps a plain
/// command name (`echo`, `ls`, `cd`) borrowed — no allocation at all.
pub(super) fn passes_through(word: &ShortCStr, mask: &[bool]) -> bool {
    word.as_bytes().is_ok_and(|bytes| {
        !has_trigger(bytes)
            && !bytes.contains(&b'\\')
            && !crate::glob::has_unquoted_pattern(bytes, mask)
    })
}

/// Word 0 as the bash field list: the whole word goes through the argument
/// substitution pipeline (`$( … )`, `` ` … ` ``, `$(( … ))`, `$name`, `%fd`,
/// `~`), is IFS-split by its own quote mask, and every field is
/// pathname-expanded — `fields[0]` is the command name, `fields[1..]` its
/// leading arguments.
///
/// A word with no trigger keeps today's word-0 path (fold the escape pairs,
/// then glob); a word with one is handed to `substitute_args`, which folds the
/// pairs in its own `\\` arms, so the two paths agree on every spelling
/// (LESSONS: the escape pair is folded at every word surface).
pub(crate) fn fields(
    word: &ShortCStr,
    mask: &[bool],
    spans: &[usize],
    quoted: bool,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, Report<CmdError>> {
    let bytes = word.as_bytes().change_context(CmdError::Never)?;
    if !has_trigger(bytes) {
        let (folded, folded_mask) = fold_word(word, mask).change_context(CmdError::Never)?;
        return crate::glob::expand(&folded, &folded_mask, cell).change_context(CmdError::Resolve);
    }
    let mask = mask.to_vec();
    let spans = spans.to_vec();
    crate::substitute::substitute_args(
        core::slice::from_ref(word),
        core::slice::from_ref(&mask),
        core::slice::from_ref(&spans),
        core::slice::from_ref(&quoted),
        cell,
    )
    .change_context(CmdError::Resolve)
}
