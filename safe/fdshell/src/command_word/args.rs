//! Word 0's *argument* side: the leading words an empty expansion consumes,
//! and the encoding that keeps word 0's trailing fields expanded-once when the
//! child re-runs the argument pipeline on `args`.

use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;

use super::fields::fields;

/// The `CommandLine` argument trio (`args`, `args_mask`, `args_quoted`) an
/// expanded word 0 produces.
pub(super) type LeadingArgs = (Vec<ShortCStr>, Vec<Vec<bool>>, Vec<bool>);

/// bash: a word 0 whose expansion is empty is not a word, so the next field
/// becomes the command — consume leading words (each expanded the same way)
/// until one yields fields. `consumed` counts the words the loop took; when the
/// words run out, the command is the empty name.
pub(super) fn consume(
    cmdline: &CommandLine,
    mut word_fields: Vec<ShortCStr>,
    cell: &ForkCell<ShellState>,
) -> Result<(Vec<ShortCStr>, usize), Report<CmdError>> {
    let mut consumed = 0usize;
    while word_fields.is_empty() {
        let Some(word) = cmdline.args.get(consumed) else {
            break;
        };
        let mask = cmdline
            .args_mask
            .get(consumed)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let quoted = cmdline.args_quoted.get(consumed).copied().unwrap_or(false);
        consumed += 1;
        word_fields = fields(word, mask, quoted, cell)?;
    }
    Ok((word_fields, consumed))
}

/// Word 0's trailing fields (quote-encoded, so the child cannot re-expand
/// them) followed by the leading words word 0 did not consume.
pub(super) fn leading_args(
    fields: impl Iterator<Item = ShortCStr>,
    cmdline: &CommandLine,
    consumed: usize,
) -> Result<LeadingArgs, Report<CmdError>> {
    let mut args = Vec::new();
    let mut args_mask = Vec::new();
    let mut args_quoted = Vec::new();
    for field in fields {
        let arg = quote_encode(&field)?;
        args_mask.push(vec![true; arg.len()]);
        args_quoted.push(true);
        args.push(arg);
    }
    args.extend_from_slice(cmdline.args.get(consumed..).unwrap_or(&[]));
    args_mask.extend_from_slice(cmdline.args_mask.get(consumed..).unwrap_or(&[]));
    args_quoted.extend_from_slice(cmdline.args_quoted.get(consumed..).unwrap_or(&[]));
    Ok((args, args_mask, args_quoted))
}

/// A word-0 field the child receives as an argument is already expanded, and
/// the child re-runs the argument pipeline on `args`, so it goes in the
/// single-quote encoding: `$`, `` ` `` and `\` as escape pairs, every mask bit
/// `true`. Both readers of word text decode those pairs back to the literal
/// bytes, and the pairs — not the mask — are what keep a `$name` literal,
/// because the parameter-expansion arm ignores the mask.
fn quote_encode(field: &ShortCStr) -> Result<ShortCStr, Report<CmdError>> {
    let mut out = ShortCStr::new();
    for b in field.as_bytes().change_context(CmdError::Never)? {
        if matches!(b, b'$' | b'`' | b'\\') {
            out.push_byte(b'\\').change_context(CmdError::Never)?;
        }
        out.push_byte(*b).change_context(CmdError::Never)?;
    }
    Ok(out)
}
