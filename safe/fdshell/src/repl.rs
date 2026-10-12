use alloc::vec::Vec;
use core::fmt::Write;
use error_stack::{Report, ResultExt};

use crate::app::AppError;
use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Origin, Position, ScriptText, ShortCStr};

mod complete;
mod control;
mod line;
mod trailing;

pub(crate) use crate::cond::run_cond_list;
pub(crate) use crate::script::run_script;

pub(super) fn handle(
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    if let Some(control) = run_script(text, cell)? {
        control::out_of_scope(control)?;
    }
    Ok(())
}

pub fn exec_cmd(text: &ScriptText, cell: &ForkCell<ShellState>) -> Result<i32, Report<CmdError>> {
    if let Some(control) = run_script(text, cell)? {
        control::out_of_scope(control)?;
    }
    let state = cell.borrow().change_context(CmdError::Never)?;
    Ok(state.last_status.exit_code())
}

pub fn run(cell: &ForkCell<ShellState>) -> Result<(), Report<AppError>> {
    // Seed `$0` with the invocation name for interactive mode. Only this path
    // needs it: `-c` and script-file modes seed it before dispatching.
    {
        let mut state = cell.borrow_mut().change_context(AppError::Borrow)?;
        state.set_program_name(ImportedStr::shell(ShortCStr::from(c"fdshell")));
        // Like bash, `ignoreeof` is on only when stdin is a terminal, so a
        // stray Ctrl+D does not kill an interactive shell; piped input
        // still exits on EOF.
        if sys::IN.is_tty().change_context(AppError::Read)? {
            state.options |= crate::options::IGNOREEOF;
        }
    }
    let mut buf = Vec::new();
    let mut input = line::LineReader::new();
    loop {
        buf.clear();
        if !line::read_line(
            cell,
            &mut buf,
            b"fdshell> ",
            crate::cmd_subst::MAX_CAPTURED,
            &mut input,
        )? {
            return Ok(());
        }
        // Buffer incomplete input (open blocks, unterminated heredocs,
        // trailing operators, unbalanced quotes) until it forms a complete
        // construct. On EOF with `ignoreeof` off, break and execute what we
        // have (bash: incomplete input at EOF runs and reports the parse
        // error); with `ignoreeof` on, `read_line` keeps the shell alive.
        while !complete::is_complete(&buf) {
            buf.push(b'\n');
            if !line::read_line(
                cell,
                &mut buf,
                b"> ",
                crate::cmd_subst::MAX_CAPTURED,
                &mut input,
            )? {
                break;
            }
        }
        let input = buf.trim_ascii();
        if input.is_empty() {
            continue;
        }
        let text = ScriptText::new(
            ShortCStr::from_vec(input.to_vec()).change_context(AppError::Read)?,
            Position::new(1, 1),
            Origin::Stdin,
        );
        if let Err(err) = handle(&text, cell) {
            let _ = writeln!(crate::io::Stderr, "{err:?}");
        }
    }
}

#[cfg(test)]
mod tests;
