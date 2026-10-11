use crate::{AppError, ShellState, exec_cmd};
use alloc::collections::VecDeque;
use core::fmt::Write;
use error_stack::{Report, ResultExt};
use sys::Origin;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Position, ScriptText, Trace};

pub fn run_cmd_mode(
    all_args: &[ShortCStr],
    state: &ForkCell<ShellState>,
) -> Result<(), Report<AppError>> {
    let cmd = all_args.get(1).ok_or(AppError::Usage)?;
    let positional: VecDeque<ImportedStr> = all_args
        .iter()
        .enumerate()
        .skip(2)
        .map(|(i, arg)| ImportedStr::new(arg.clone(), Trace::boundary(Origin::CliArgument(i + 1))))
        .collect();
    {
        let mut state = state.borrow_mut().change_context(AppError::Borrow)?;
        // bash argv semantics: `$0` is the invocation name and never a
        // positional; the words after the command string are `$1..`.
        let name = if positional.is_empty() {
            c"sh"
        } else {
            c"fdshell"
        };
        state.set_program_name(ImportedStr::shell(ShortCStr::from(name)));
        state.set_positional(positional);
    }
    let text = ScriptText::new(cmd.clone(), Position::new(1, 1), Origin::CliArgument(2));
    match exec_cmd(&text, state) {
        Ok(code) => {
            if code != 0 {
                sys::exit(code);
            }
            Ok(())
        }
        Err(info) => {
            let _ = writeln!(crate::io::Stderr, "{info:?}");
            sys::exit(1);
        }
    }
}

pub fn execute_script(
    script_content: &[u8],
    origin: Origin,
    state: &ForkCell<ShellState>,
) -> Result<(), Report<AppError>> {
    let text = ScriptText::new(
        ShortCStr::from_vec(script_content.to_vec()).change_context(AppError::Never)?,
        Position::new(1, 1),
        origin,
    );
    match exec_cmd(&text, state) {
        Ok(code) => {
            if code != 0 {
                sys::exit(code);
            }
            Ok(())
        }
        Err(info) => {
            let _ = writeln!(crate::io::Stderr, "{info:?}");
            sys::exit(1);
        }
    }
}
