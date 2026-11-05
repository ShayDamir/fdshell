//! Word 0 (the command name) is a word like any other: it goes through the
//! shared word-expansion pipeline in the parent, before any dispatch sees it.
//!
//! bash expands the command word before looking it up, so a whole-word command
//! substitution names the command (`$(echo echo) hi` runs `echo`), a mid-word
//! one concatenates with the literal bytes (`$(echo "ec")ho`), the expansion is
//! IFS-split and pathname-expanded, the first field is the command and the rest
//! are its leading arguments. Expanding here — one call in `run_parsed`, which
//! every statement path passes through — is what feeds the function lookup, the
//! intercept dispatch, the `command`/`builtin` prefix resolution and the `PATH`
//! search with the expanded name.

mod args;
mod fields;

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::parse::{CommandLine, ExpandedStage, Pipeline, Stage};
use crate::state::ShellState;

pub(crate) use fields::fields;

/// Word 0 expanded: `Cow::Borrowed` when the word is its own single field (the
/// common command line, allocation-free), `Cow::Owned` carrying the expanded
/// command name and its leading arguments folded into `args` otherwise.
pub(crate) fn expand<'a>(
    cmdline: &'a CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<Cow<'a, CommandLine>, Report<CmdError>> {
    // The per-command scratch (task #426): the vanished-word marker and the last
    // substitution's status are the run side's decision inputs, and each command
    // — each pipeline stage — answers with what **its own** expansion produced.
    {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.word0_vanished = false;
        state.last_substitution = None;
    }
    if fields::passes_through(&cmdline.command, &cmdline.command_mask) {
        return Ok(Cow::Borrowed(cmdline));
    }
    let word = fields(
        &cmdline.command,
        &cmdline.command_mask,
        &cmdline.command_span,
        cmdline.command_quoted,
        cell,
    )?;
    Ok(Cow::Owned(fields::with_fields(cmdline, word, cell)?))
}

/// Word 0 of every pipeline stage (a pipeline launches as a whole, so its
/// stages are expanded here, in stage order). A keyword-block stage has no word
/// 0 at all (task #292), so it is borrowed as it is: its inner commands expand
/// in the child that runs the block.
pub(crate) fn expand_pipeline<'a>(
    pipeline: &'a Pipeline,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ExpandedStage<'a>>, Report<CmdError>> {
    let mut stages = Vec::new();
    for stage in &pipeline.stages {
        match stage {
            Stage::Command(cmd, text) => {
                let expanded = expand(cmd, cell)?.into_owned();
                stages.push(ExpandedStage::Command(Box::new(expanded), text.clone()));
            }
            Stage::Block(block) => stages.push(ExpandedStage::Block(block)),
        }
    }
    Ok(stages)
}

#[cfg(test)]
mod tests;
