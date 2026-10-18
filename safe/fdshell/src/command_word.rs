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

use args::{consume, leading_args};

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::parse::{CommandLine, Pipeline};
use crate::state::ShellState;

pub(crate) use fields::fields;

/// Word 0 expanded: `Cow::Borrowed` when the word is its own single field (the
/// common command line, allocation-free), `Cow::Owned` carrying the expanded
/// command name and its leading arguments folded into `args` otherwise.
pub(crate) fn expand<'a>(
    cmdline: &'a CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<Cow<'a, CommandLine>, Report<CmdError>> {
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
    Ok(Cow::Owned(with_fields(cmdline, word, cell)?))
}

/// Word 0 of every pipeline stage (a pipeline launches as a whole, so its
/// stages are expanded here, in stage order).
pub(crate) fn expand_pipeline(
    pipeline: &Pipeline,
    cell: &ForkCell<ShellState>,
) -> Result<Pipeline, Report<CmdError>> {
    let mut commands = Vec::new();
    for cmdline in &pipeline.commands {
        commands.push(expand(cmdline, cell)?.into_owned());
    }
    Ok(Pipeline { commands })
}

/// Rebuild the command line from word 0's field list: `fields[0]` is the
/// command name (the empty name included — it is what bash reports as
/// `: command not found`), its mask fully quoted so the child's word-0 rules
/// are no-ops on it, and `fields[1..]` become leading arguments.
fn with_fields(
    cmdline: &CommandLine,
    fields: Vec<ShortCStr>,
    cell: &ForkCell<ShellState>,
) -> Result<CommandLine, Report<CmdError>> {
    let (fields, consumed) = consume(cmdline, fields, cell)?;
    let mut fields = fields.into_iter();
    let command = fields.next().unwrap_or_else(ShortCStr::new);
    let (args, args_mask, args_span, args_quoted) = leading_args(fields, cmdline, consumed)?;
    Ok(CommandLine {
        prefix: cmdline.prefix,
        command: command.clone(),
        command_mask: vec![true; command.len()],
        command_span: Vec::new(),
        command_quoted: true,
        env_assigns: cmdline.env_assigns.clone(),
        args,
        args_mask,
        args_span,
        args_quoted,
        captures: cmdline.captures.clone(),
        redirects: cmdline.redirects.clone(),
        pidvar: cmdline.pidvar.clone(),
        bg_force: cmdline.bg_force,
    })
}

#[cfg(test)]
mod tests;
