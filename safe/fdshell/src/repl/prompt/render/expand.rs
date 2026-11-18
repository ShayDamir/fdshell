// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! Expanding one literal run of the decoded prompt — the stage bash runs after
//! decoding (parameter expansion, command substitution, arithmetic expansion and
//! quote removal), behind the `promptvars` shopt, which is on by default
//! (measured `shopt promptvars` → `promptvars on`). The renderer calls it per
//! run; the entry point it calls is `substitute::substitute_prompt_run`.
//!
//! This is the second escape/expansion grammar in the tree (LESSONS: "The two
//! escape decoders have different grammars, and the record keeps the boundary").
//! Every rule below was measured against bash 5.3.9 on 2026-11-18:
//!
//! - no tilde arm (`~ ` draws `~ `), no field splitting (`a  b` keeps both
//!   spaces), no pathname expansion (`*` draws `*`), no separators eaten (`a;b`,
//!   `a|b` keep theirs);
//! - quote characters are literal **data** and do not protect: `"$v"` draws
//!   `"V"` and `a"b$v"c` draws `a"bV"c`. No tokenizer ran here, so the word has
//!   no quote spans and its mask carries no region information;
//! - the backslash is a quote operator only before `$ \`" ' \`, and keeps its own
//!   byte before anything else: `a\"b` draws `a"b`, `a\\b` draws `a\b`, `\qX`
//!   draws `\qX`;
//! - a byte the **decoder** produced is part of the run and is scanned, so
//!   `\044v` draws `V` — an octal-decoded `$` is a live trigger. The `\xHH`
//!   extension (decision D1 of task #616) therefore expands where bash stays
//!   literal; the divergence is the decoder's, and this stage only inherits it.
//! - a NUL byte (only `\000` / `\x00` reach a run) splits it: each side is
//!   expanded, the NUL is pasted. `ShortCStr` — the engine's input type — is
//!   NUL-free by invariant, and a NUL stays a domain error only at the entry
//!   points that ingest outside bytes (LESSONS: "A NUL byte is reachable only at
//!   the entry points that ingest outside bytes").
//!
//! The mask is the surface's policy, supplied as data (task #371's pattern):
//! every byte unquoted, since nothing was tokenized and no quote protects —
//! except a backslash that is **not** a quote operator, whose bit routes it to
//! the arm that keeps the backslash. A name arm can never span such a bit: `\`
//! is not a name byte, so the byte that closes the region is the byte that ends
//! the name, exactly as bash's name scanner ends it.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::state::ShellState;
use crate::substitute::substitute_prompt_run;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// The bytes a backslash quotes on this surface — the ones special in some
/// quoting context (POSIX #4.2's set plus the two quote characters; measured).
const QUOTE_OPERATORS: &[u8] = b"$`\"'\\";

/// Expand one literal run of the prompt. The caller renders the result as the
/// run's final bytes; a failure is the caller's to report (task #620's D-620-8:
/// bash reports and keeps the unexpanded text).
pub(super) fn expand_run(
    bytes: &[u8],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<u8>, Report<ResolveError>> {
    let mut out = Vec::new();
    for (i, piece) in bytes.split(|&byte| byte == 0).enumerate() {
        if i > 0 {
            out.push(0);
        }
        out.extend(
            expand_piece(piece, cell)?
                .as_bytes()
                .change_context(ResolveError::Never)?,
        );
    }
    Ok(out)
}

/// Expand one NUL-free piece with the surface's mask and no quote spans.
fn expand_piece(
    piece: &[u8],
    cell: &ForkCell<ShellState>,
) -> Result<ShortCStr, Report<ResolveError>> {
    let run = ShortCStr::from_vec(piece.to_vec()).change_context(ResolveError::NulByte)?;
    substitute_prompt_run(&run, &mask(piece), cell)
}

/// The run's mask: [`MaskBit::Unquoted`] everywhere but the backslashes the
/// module doc says are not quote operators, whose bit routes them to the arm
/// that keeps the backslash.
fn mask(piece: &[u8]) -> Vec<MaskBit> {
    piece
        .iter()
        .enumerate()
        .map(|(i, &byte)| {
            let escaped = byte == b'\\';
            let operator = piece
                .get(i + 1)
                .is_some_and(|next| QUOTE_OPERATORS.contains(next));
            if escaped && !operator {
                MaskBit::Quoted
            } else {
                MaskBit::Unquoted
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
