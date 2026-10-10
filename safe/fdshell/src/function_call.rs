use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::parse::BuiltinPrefix;
use crate::state::ShellState;
use alloc::collections::VecDeque;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ScriptText, ShortCStr, Trace};

/// Run a user function call, executing `body` in this shell. Arguments replace
/// the positional parameters for the duration of the call. `look_up` resolves
/// the body once in the caller, which owns the command's redirect scope.
pub(crate) fn call(
    text: &ScriptText,
    cmdline: &crate::parse::CommandLine,
    body: &ShortCStr,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let substituted = crate::substitute::substitute_args(
        &cmdline.args,
        &cmdline.args_mask,
        &cmdline.args_quoted,
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let saved = swap_positional(cell, &cmdline.command, &substituted, text)?;
    crate::state::frames::push_frame(cell)?;
    let script = ScriptText::new(body.clone(), text.start, text.origin.clone());
    let result = crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
        crate::script::run_script(&script, cell)
    });
    // Restore the caller's positional parameters and the `local` frame before
    // propagating, so a body that fails (or `return`s out) leaks nothing.
    restore_positional(cell, saved)?;
    crate::state::frames::pop_frame(cell)?;
    let mut control = result?;
    if matches!(control, Some(LoopControl::Return)) {
        control = None;
    }
    Ok(control)
}

/// The stored body of the function named by `cmdline`, or `None`. A
/// `builtin`/`command` prefix bypasses function lookup.
pub(crate) fn look_up(
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<Option<ShortCStr>, Report<CmdError>> {
    if cmdline.prefix != BuiltinPrefix::None {
        return Ok(None);
    }
    let state = cell.borrow().change_context(CmdError::Never)?;
    Ok(state.functions.get(&cmdline.command).cloned())
}

/// Replace the positional parameters so the function sees `$0` as its name and
/// `$1..` as its arguments, returning the saved parameters.
fn swap_positional(
    cell: &ForkCell<ShellState>,
    name: &ShortCStr,
    args: &[ShortCStr],
    text: &ScriptText,
) -> Result<VecDeque<ImportedStr>, Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let saved = core::mem::take(&mut state.positional);
    let trace = Trace::at(text.start, text.origin.clone());
    let mut positional = VecDeque::new();
    positional.push_back(ImportedStr::new(name.clone(), trace.clone()));
    for s in args {
        positional.push_back(ImportedStr::new(s.clone(), trace.clone()));
    }
    state.set_positional(positional);
    Ok(saved)
}

fn restore_positional(
    cell: &ForkCell<ShellState>,
    saved: VecDeque<ImportedStr>,
) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_positional(saved);
    Ok(())
}

#[cfg(test)]
mod tests;
