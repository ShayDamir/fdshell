// mutants: -C --lib -C --test -C colon -C --test -C unset_expand -C --test -C positional -C --test -C arith -C --test -C quotes -C --test -C cmd_subst -C --test -C redirect -- -E 'test(/^substitute::/) | test(/^command_word::/) | binary(colon) | binary(unset_expand) | binary(positional) | binary(arith) | binary(quotes) | binary(cmd_subst) | binary(redirect)'

mod arg;
mod array_fields;
mod backtick;
mod brace;
mod dollar;
mod fields;
mod join;
mod mask;
mod origin;
mod param_op;
mod paren;
pub(crate) mod pattern;
mod percent;
mod positional;
pub(crate) mod resolve;
mod split;
mod subst_paren;
mod surface;
mod tilde;
mod words;

pub use words::substitute_args;

pub(crate) use fields::word_fields;
pub(crate) use join::Join;
pub(crate) use surface::PercentArm;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::fork_cell::{ForkCell, Ref};
use sys::{ExportedFd, ShortCStr};

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::state::ShellState;

/// Substitute one word, discarding the byte provenance: the wrapper for
/// every caller that does not split the result (assignments, heredocs,
/// `local`/`let`, `case` — the value is stored whole, LESSONS). `spans` is
/// the word's zero-byte quote-span list, parallel to the mask (task #321).
/// Every surface behind this wrapper is a `%`-arm surface (task #371), so the
/// wrapper pins the default policy; the one `Off` surface (the redirect
/// target) drives the field pipeline through `word_fields` itself.
pub(crate) fn substitute_arg(
    arg: &ShortCStr,
    mask: &[MaskBit],
    spans: &[usize],
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<(ShortCStr, Vec<MaskBit>), Report<ResolveError>> {
    let (out, out_mask, _origin) =
        arg::substitute_arg_marked(arg, mask, spans, Join::Args, PercentArm::Fires, cache, cell)?;
    Ok((out, out_mask))
}

pub(crate) fn borrow_state(
    cell: &ForkCell<ShellState>,
) -> Result<Ref<'_, ShellState>, Report<ResolveError>> {
    cell.borrow().change_context(ResolveError::RefNotFound)
}

/// A word is fully quoted when every byte was consumed inside double quotes,
/// or when the word is empty — an empty mask only arises from quoted
/// material (a quoted empty word like `""`, or a quoted expansion that
/// produced nothing), which is one word even when empty.
pub(super) fn fully_quoted(mask: &[MaskBit]) -> bool {
    mask.is_empty() || mask.iter().copied().all(MaskBit::quoted)
}

/// A word whose bytes can start an **expansion**: `$`, a backtick, a `%` of a
/// live arm, or a leading `~`. This is a **raw-text** test, never a mask test:
/// `"$(echo ls)"` carries mask `true` on every byte and must still expand
/// (bash doubles quotes do not suppress command substitution), so a
/// mask-gated word is an unsound false negative. A false positive is harmless
/// — every arm of the pipeline is correct on a word with no trigger (and on a
/// quoted `%`, which the arm's own mask-bit gate leaves literal, task #252).
/// `percent` is the surface's arm policy (task #371): a word enters
/// substitution when it carries a trigger byte of an arm that is **live** on
/// this surface. On the argument surface the `%` arm fires, so the word
/// pipeline owns the `%name` lookup. On the **redirect target** it is off —
/// measured in bash 5.3.9, no `%` form expands in a redirection word
/// (`> a%b` → the file `a%b`, `> a%CWD/x` → `ENOENT` for that literal name),
/// and fdshell decides every `%fd` redirect target at **parse** time — the
/// attached `>%name` is a `RedirectSource::Var`, the spaced `%name` is
/// `invalid redirect syntax` — so a `%` byte in a path word is file-name data.
/// Keeping `%` out of the gate keeps `> a%b` and `> a%CWD/x` at bash's answer
/// and keeps the no-trigger fast path for `%`-only words; a word that carries
/// *another* trigger reaches the substitution with the arm itself off, so
/// `> a%log$(echo x)` writes the literal `a%logx` exactly as bash does.
pub(crate) fn has_trigger(bytes: &[u8], percent: PercentArm) -> bool {
    leads_tilde(bytes)
        || bytes
            .iter()
            .any(|b| matches!(b, b'$' | b'`') || (*b == b'%' && percent == PercentArm::Fires))
}

/// The tilde trigger is positional: only a **leading** `~` can expand
/// (POSIX #2.6.1, and `substitute/tilde.rs` is the arm).
fn leads_tilde(bytes: &[u8]) -> bool {
    bytes.first() == Some(&b'~')
}

#[cfg(test)]
mod tests;
