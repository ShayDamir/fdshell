//! Scoped `NAME=value` command prefixes (POSIX 2.9.1): the values are
//! expanded against the current state, applied to the command's environment
//! (exported to external children), and never persisted in the shell.

mod apply;

pub use apply::{apply, apply_child, restore};

use crate::error::cmd::CmdError;
use crate::parse::{CommandLine, Pipeline};
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ScriptText, ShortCStr, Trace};

/// One expanded assignment word; application is then a plain map write.
#[derive(Clone)]
pub struct EnvAssign {
    name: ShortCStr,
    value: ImportedStr,
}

pub type EnvAssigns = Vec<EnvAssign>;

/// Expand the command's prefix assignment words against the current state.
/// No cell borrow is held across a value (`$(…)` in a value forks).
pub fn expand(
    cmdline: &CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<EnvAssigns, Report<CmdError>> {
    let mut out = Vec::new();
    for (name, value) in &cmdline.env_assigns {
        out.push(expand_one(name, value, text, cell)?);
    }
    Ok(out)
}

/// Expand every pipeline stage's prefix, in stage order.
pub fn expand_pipeline(
    pipeline: &Pipeline,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<EnvAssigns>, Report<CmdError>> {
    let mut out = Vec::new();
    for cmd in &pipeline.commands {
        out.push(expand(cmd, text, cell)?);
    }
    Ok(out)
}

/// Expand one raw value word and trace its origin like a shell assignment.
fn expand_one(
    name: &ShortCStr,
    value: &ShortCStr,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<EnvAssign, Report<CmdError>> {
    let (expanded, _) = crate::substitute::substitute_arg(value, &[], &mut HashMap::new(), cell)
        .change_context(CmdError::Resolve)?;
    let origin = crate::run_origin::assign_origin(value, text.origin.clone(), cell)?;
    Ok(EnvAssign {
        name: name.clone(),
        value: ImportedStr::new(expanded, Trace::at(text.start, origin)),
    })
}

#[cfg(test)]
mod tests;
