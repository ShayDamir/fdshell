// mutants: -C --lib -C --test -C colon -C --test -C unset_expand -C --test -C positional -C --test -C arith -C --test -C quotes -- -E 'test(/^substitute::/) | binary(colon) | binary(unset_expand) | binary(positional) | binary(arith) | binary(quotes)'

use error_stack::Report;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::state::ShellState;
use crate::substitute::Join;
use crate::substitute::origin::Origin;

mod op;
mod scan;
mod word;

pub(in crate::substitute) use op::ParamOp;
pub(super) use scan::split_operator;
use word::Word;

pub(super) mod colon;
pub(super) mod pattern;

/// The join the operator word's `$@`/`$*` uses, per the rows measured in bash
/// 5.3.9: `:=` stores the scalar it measured, so it joins into one text; `:?`
/// reports a message, which always space-joins; `:-` and `:+` produce a value
/// for the word surface, so they keep [`Join::Args`].
fn join_of(op: ParamOp) -> Join {
    match op {
        ParamOp::Assign => Join::Assign,
        ParamOp::Error => Join::Message,
        _ => Join::Args,
    }
}

/// Applies the operator, appending the expansion to `out`. `mask` is the word's
/// quote mask, `spans` its zero-byte quote spans and `content_start` the word
/// index of the first braced-content byte, so each arm can slice all three under
/// its own bytes. The colon arms hand their word to `word.rs`, which runs it
/// through the ordinary word pipeline first (task #561); the pattern arm keeps
/// its own mask slice, since a pattern is never expanded. `origin` is the outer
/// word's provenance, which replays a quoted `$@`'s boundaries.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_param_op(
    name: &ShortCStr,
    op: ParamOp,
    source: &ShortCStr,
    mask: &[MaskBit],
    content_start: usize,
    spans: &[usize],
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    origin: &mut Origin,
) -> Result<(), Report<ResolveError>> {
    let w = Word {
        literal: source,
        mask,
        start: content_start + name.len() + op.word_offset(),
        spans,
        cell,
        join: join_of(op),
        force_unquoted: op == ParamOp::Error,
    };
    match op {
        ParamOp::Default | ParamOp::Alternate | ParamOp::Error => {
            colon::apply(name, op, &w, cell, out, origin)
        }
        ParamOp::Assign => colon::assign(name, &w, cell, out, origin),
        ParamOp::ShortestPrefix
        | ParamOp::LongestPrefix
        | ParamOp::ShortestSuffix
        | ParamOp::LongestSuffix => {
            pattern::apply(name, op, source, mask, content_start, cell, out)
        }
    }
}
