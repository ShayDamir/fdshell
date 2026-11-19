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
pub(crate) mod surface;
mod tilde;
mod words;

pub use words::substitute_args;

pub(crate) use fields::word_fields;
pub(crate) use join::Join;
pub(crate) use surface::{PercentArm, TildeArm};

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
    let (out, out_mask, _origin) = arg::substitute_arg_marked(
        arg,
        mask,
        spans,
        Join::Args,
        PercentArm::Fires,
        TildeArm::Fires,
        cache,
        cell,
    )?;
    Ok((out, out_mask))
}

/// The **prompt** surface's entry point (task #620), the prompt renderer's only
/// caller: one literal run of the decoded prompt, expanded by parameter
/// expansion, command substitution, arithmetic expansion and quote removal —
/// and nothing else. No tilde arm (measured: `PS1='~ '` draws `~ `), no field
/// splitting, no pathname expansion, quote characters as literal data; the
/// grammar is recorded in `repl/prompt/render/expand.rs`. `mask` is the run's
/// own mask — every byte unquoted, since nothing here was tokenized and no quote
/// protects — except the bits the surface's grammar routes to another arm
/// (expand.rs states which). `spans` is empty: a zero-byte quote span is the
/// tokenizer's, and this surface has none. The output mask and provenance come
/// back unused because nothing splits or globs the text; `Join::Pattern` is the
/// one join mode reproducing the prompt's measured `$@`/`$*` rows (D-620-6).
pub(crate) fn substitute_prompt_run(
    run: &ShortCStr,
    mask: &[MaskBit],
    cell: &ForkCell<ShellState>,
) -> Result<ShortCStr, Report<ResolveError>> {
    let mut cache = HashMap::new();
    let (out, _out_mask, _origin) = arg::substitute_arg_marked(
        run,
        mask,
        &[],
        Join::Pattern,
        PercentArm::Fires,
        TildeArm::Off,
        &mut cache,
        cell,
    )?;
    Ok(out)
}

pub(crate) fn borrow_state(
    cell: &ForkCell<ShellState>,
) -> Result<Ref<'_, ShellState>, Report<ResolveError>> {
    cell.borrow().change_context(ResolveError::RefNotFound)
}

#[cfg(test)]
mod tests;
