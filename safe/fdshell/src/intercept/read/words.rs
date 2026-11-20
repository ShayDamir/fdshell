// mutants: -C --lib -C --test -C read -C --test -C trap -- -E 'test(/^intercept::read::tests::/) | binary(read) | binary(trap)'

use alloc::vec::Vec;

use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use crate::substitute::substitute_args;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// The `-p` operand, expanded: bash decodes prompt escapes only for its four
/// prompt drawers, so the operand is an **ordinary word** — the normal word
/// pipeline runs (substitution, `IFS` splitting, glob), and the operand slot
/// takes the **first** field. The pipeline runs whether or not the prompt is
/// drawn, exactly as bash's parse-time expansion does. Only this word: which
/// fields the other words become is the expansion-policy task's question.
pub(crate) fn prompt_operand(
    index: usize,
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<ShortCStr, Report<CmdError>> {
    let word = cmdline.args.get(index).cloned().unwrap_or_default();
    let masks = cmdline.args_mask.get(index).cloned().unwrap_or_default();
    let spans = cmdline.args_span.get(index).cloned().unwrap_or_default();
    let quoted = cmdline.args_quoted.get(index).copied().unwrap_or(false);
    let fields = substitute_args(
        core::slice::from_ref(&word),
        core::slice::from_ref(&masks),
        core::slice::from_ref(&spans),
        core::slice::from_ref(&quoted),
        cell,
    )
    .change_context(CmdError::Resolve)?;
    Ok(fields.into_iter().next().unwrap_or_default())
}

/// The default IFS's whitespace, stripped at the line's edges before any field
/// is assigned. The asymmetry with `CUT` below is deliberate: newline trims but
/// does not cut (driving the cut set from `state.ifs` is task #158), and CR is
/// content, not IFS whitespace.
const TRIM: &[u8] = b" \t\n";

/// The bytes this splitter cuts fields on — the hardcoded default-IFS set,
/// pending task #158. A run of them is one boundary; the text between the
/// fields the last variable receives is kept verbatim, never rejoined.
const CUT: &[u8] = b" \t";

pub(crate) fn split_fields(data: &[u8], num_targets: usize) -> Vec<Vec<u8>> {
    let line = trim_edges(data);
    let ranges = field_ranges(line);
    if ranges.len() < num_targets {
        return pad(&ranges, line, num_targets);
    }

    head_and_tail(&ranges, line, num_targets)
}

/// Drop the leading and trailing `TRIM` bytes.
fn trim_edges(data: &[u8]) -> &[u8] {
    let mut core = data;
    while let Some((b, rest)) = core.split_first() {
        core = if TRIM.contains(b) { rest } else { break };
    }
    while let Some((b, rest)) = core.split_last() {
        core = if TRIM.contains(b) { rest } else { break };
    }

    core
}

/// The `(start, end)` range of each non-empty field; after the trim, no field
/// is empty, and the trailing run that once produced a phantom field is gone.
fn field_ranges(line: &[u8]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for (i, &b) in line.iter().enumerate() {
        if CUT.contains(&b) {
            if i > start {
                ranges.push((start, i));
            }
            start = i + 1;
        }
    }
    if start < line.len() {
        ranges.push((start, line.len()));
    }

    ranges
}

/// Fewer fields than targets: every field keeps its own text, the missing
/// targets get empty values.
fn pad(ranges: &[(usize, usize)], line: &[u8], num_targets: usize) -> Vec<Vec<u8>> {
    let mut fields = Vec::with_capacity(num_targets);
    for &(start, end) in ranges {
        if let Some(field) = line.get(start..end) {
            fields.push(field.to_vec());
        }
    }
    while fields.len() < num_targets {
        fields.push(Vec::new());
    }

    fields
}

/// The first `num_targets - 1` fields individually, then the last target takes
/// the trimmed line's tail from its first field's start — internal separators
/// included. `num_targets == 1` takes the whole trimmed line, and an exact
/// field count is this rule's degenerate case. `num_targets >= 1` is
/// guaranteed by `collect_targets`.
fn head_and_tail(ranges: &[(usize, usize)], line: &[u8], num_targets: usize) -> Vec<Vec<u8>> {
    let mut result: Vec<Vec<u8>> = Vec::with_capacity(num_targets);
    for &(start, end) in ranges.iter().take(num_targets - 1) {
        if let Some(field) = line.get(start..end) {
            result.push(field.to_vec());
        }
    }

    if let Some(&(start, _)) = ranges.get(num_targets - 1)
        && let Some(tail) = line.get(start..)
    {
        result.push(tail.to_vec());
    }

    result
}
