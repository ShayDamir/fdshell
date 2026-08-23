use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ScriptText, ShortCStr};

/// `eval`: join the substituted args and run them as a script in this shell.
pub(crate) fn run_eval(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    super::validation::validate_intercept(line, "eval", cmdline)?;
    let substituted = crate::substitute::substitute_args(&cmdline.args, &cmdline.args_fq, cell)
        .change_context(CmdError::Resolve)?;
    let script = join_space(&substituted);
    if script.is_empty() {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_last_exit(0);
        return Ok(None);
    }
    let script_text = ScriptText::new(script, text.start, text.origin.clone());
    super::last_arg_frame::with_eval_frame(cell, || {
        // Count each eval level toward the nesting cap (a self-eval would
        // otherwise recurse through run_script until the stack overflows).
        crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
            crate::script::run_script(&script_text, cell)
        })
    })
}

fn join_space(args: &[ShortCStr]) -> ShortCStr {
    let mut out = ShortCStr::new();
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            out.push(c" ");
        }
        out.push(arg);
    }
    out
}

#[cfg(test)]
mod tests;
